//! Pipe-to-MCP adapter: exposes seaki pipe commands as MCP tools.

use std::collections::HashMap;

use seaki_pipe::ast::{ComposedPipeline, ComposedStep, FailurePolicy, InputBinding};
use seaki_pipe::registry::{CommandRegistry, PipeCommandManifest};
use seaki_pipe::run::{run, CommandExecutor, ExecutionContext, ResourceUsage, StepPolicy};
use seaki_policy::PolicyDecision;
use serde_json::Value;

use crate::mcp::protocol::*;

/// Adapter that converts seaki pipe commands to MCP tools.
pub struct PipeToMcpAdapter;

/// Internal commands that should not be exposed as MCP tools.
const INTERNAL_COMMANDS: &[&str] = &["filter", "map", "tee", "branch", "join", "exit"];

impl PipeToMcpAdapter {
    /// Convert all non-internal commands in a registry to MCP tools.
    pub fn from_registry(registry: &CommandRegistry) -> Vec<McpTool> {
        registry
            .list()
            .into_iter()
            .filter(|m| {
                !m.command_id.trim().is_empty()
                    && !INTERNAL_COMMANDS.contains(&m.command_id.as_str())
            })
            .map(Self::manifest_to_tool)
            .collect()
    }

    /// Convert a single PipeCommandManifest to an McpTool.
    pub fn manifest_to_tool(manifest: &PipeCommandManifest) -> McpTool {
        let name = manifest
            .command_id
            .strip_prefix("mcp:")
            .unwrap_or(&manifest.command_id)
            .to_string();
        McpTool {
            name,
            description: Some(manifest.description.clone()),
            input_schema: manifest.input_schema.clone(),
        }
    }
}

struct AllowPolicy;

impl StepPolicy for AllowPolicy {
    fn check(&self, _step: &ComposedStep, _ctx: &ExecutionContext) -> PolicyDecision {
        PolicyDecision::Allow
    }
}

/// A lightweight MCP server that exposes seaki pipe commands as MCP tools.
pub struct PipeMcpServer {
    registry: CommandRegistry,
    executors: HashMap<String, Box<dyn CommandExecutor>>,
}

impl PipeMcpServer {
    /// Create a new server with the given registry and executors.
    pub fn new(
        registry: CommandRegistry,
        executors: HashMap<String, Box<dyn CommandExecutor>>,
    ) -> Self {
        Self {
            registry,
            executors,
        }
    }

    /// Handle a raw JSON-RPC request string and return a JSON-RPC response string.
    pub fn handle_request(&mut self, request_json: &str) -> Result<String, McpError> {
        let request: JsonRpcRequest<Value> = serde_json::from_str(request_json)
            .map_err(|e| McpError::Protocol(format!("invalid JSON-RPC request: {e}")))?;

        match request.method.as_str() {
            "tools/list" => {
                let response = self.handle_tools_list(request.id)?;
                serde_json::to_string(&response).map_err(|e| McpError::Protocol(e.to_string()))
            }
            "tools/call" => {
                let response = self.handle_tools_call(request.id, request.params)?;
                serde_json::to_string(&response).map_err(|e| McpError::Protocol(e.to_string()))
            }
            _ => {
                let response = JsonRpcResponse::<Value> {
                    jsonrpc: "2.0".to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("method not found: {}", request.method),
                        data: None,
                    }),
                };
                serde_json::to_string(&response).map_err(|e| McpError::Protocol(e.to_string()))
            }
        }
    }

    fn handle_tools_list(&self, id: u64) -> Result<JsonRpcResponse<Value>, McpError> {
        let tools = PipeToMcpAdapter::from_registry(&self.registry);
        let result = serde_json::json!({ "tools": tools });
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        })
    }

    fn handle_tools_call(
        &mut self,
        id: u64,
        params: Value,
    ) -> Result<JsonRpcResponse<Value>, McpError> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| McpError::Protocol("missing name parameter".to_string()))?;
        let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

        let result = self.execute_tool(name, arguments)?;
        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(
                serde_json::to_value(result).map_err(|e| McpError::Protocol(e.to_string()))?,
            ),
            error: None,
        })
    }

    /// Execute a single tool by name with given arguments.
    pub fn execute_tool(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<McpToolResult, McpError> {
        let command_id = tool_name.to_string();

        let manifest = self
            .registry
            .inspect(&command_id)
            .map_err(|_| McpError::ToolNotFound(tool_name.to_string()))?;

        let step = ComposedStep {
            step_id: "mcp-step-1".to_string(),
            command_id: command_id.clone(),
            input_type: manifest.input_frame,
            output_type: manifest.output_frame,
            input_binding: InputBinding::Constant(arguments),
            failure_policy: FailurePolicy::FailFast,
            side_effect_level: manifest.side_effect_level,
            args: Value::Null,
        };

        let pipeline = ComposedPipeline {
            pipeline_id: format!("mcp:{tool_name}"),
            steps: vec![step],
            input_type: manifest.input_frame,
            output_type: manifest.output_frame,
        };

        let mut ctx = ExecutionContext {
            workspace_id: "mcp".to_string(),
            actor_id: "mcp-client".to_string(),
            pipeline_id: pipeline.pipeline_id.clone(),
            audit: Vec::new(),
            resource_used: ResourceUsage::default(),
            checkpoint_outputs: std::collections::HashMap::new(),
        };

        let allow_policy = AllowPolicy;
        let result = run(
            &pipeline,
            Value::Null,
            &self.registry,
            &self.executors,
            &allow_policy,
            &mut ctx,
        );

        match result {
            Ok(run_result) => {
                let content: Vec<McpContent> = run_result
                    .output
                    .into_iter()
                    .map(|frame| McpContent::Text {
                        text: frame.payload.to_string(),
                    })
                    .collect();
                Ok(McpToolResult {
                    content,
                    is_error: false,
                })
            }
            Err(_) => Ok(McpToolResult {
                content: vec![McpContent::Text {
                    text: "execution failed".to_string(),
                }],
                is_error: true,
            }),
        }
    }
}
