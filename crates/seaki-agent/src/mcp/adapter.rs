//! MCP-to-Pipe adapter: registers MCP tools as pipe commands and executes them.

use std::sync::Mutex;

use seaki_pipe::ast::{ComposedStep, FrameType};
use seaki_pipe::dry_run::FrameEnvelope;
use seaki_pipe::dry_run::{ErrorKind, PipelineError};
use seaki_pipe::registry::{CommandRegistry, ResourceQuota, SideEffectLevel};
use seaki_pipe::run::{CommandExecutor, ExecutionContext};
use seaki_pipe::Cardinality;
use serde_json::json;

use super::client::McpClient;
use super::protocol::McpError;
use super::transport::McpTransport;

/// Adapter that converts an MCP server's tools into pipe commands.
pub struct McpToPipeAdapter {
    server_name: String,
}

impl McpToPipeAdapter {
    /// Create a new adapter for the given MCP server name.
    #[must_use]
    pub fn new(server_name: String) -> Self {
        Self { server_name }
    }

    /// List tools from the client and register them in the command registry.
    ///
    /// # Errors
    /// Returns `McpError` if the client communication fails.
    pub fn register_tools<T: McpTransport>(
        &self,
        client: &mut McpClient<T>,
        registry: &mut CommandRegistry,
    ) -> Result<Vec<String>, McpError> {
        let tools = client.list_tools()?;
        let output_schema = json!({
            "type": "object",
            "properties": {
                "content": { "type": "array" },
                "is_error": { "type": "boolean" }
            },
            "required": ["content"]
        });
        let mut registered = Vec::new();

        for tool in tools {
            let command_id = format!("mcp:{}:{}", self.server_name, tool.name);
            let input_schema = tool.input_schema;
            let schema_hash = seaki_pipe::registry::PipeCommandManifest::compute_schema_hash(
                &input_schema,
                &output_schema,
            );

            let manifest = seaki_pipe::registry::PipeCommandManifest {
                command_id: command_id.clone(),
                description: tool.description.unwrap_or_default(),
                input_schema,
                output_schema: output_schema.clone(),
                input_frame: (FrameType::JsonValue, Cardinality::One),
                output_frame: (FrameType::JsonValue, Cardinality::One),
                side_effect_level: SideEffectLevel::ExternalIrreversible,
                resource_quota: Some(ResourceQuota {
                    cpu_ms: 5000,
                    memory_mb: 64,
                }),
                schema_hash,
            };

            registry
                .register(manifest)
                .map_err(|e| McpError::Protocol(format!("register failed: {e}")))?;
            registered.push(command_id);
        }

        Ok(registered)
    }

    /// Create an executor for a specific MCP tool.
    #[must_use]
    pub fn create_executor<T: McpTransport + Send + 'static>(
        &self,
        tool_name: String,
        client: McpClient<T>,
    ) -> McpCommandExecutor<T> {
        McpCommandExecutor::new(tool_name, client)
    }
}

/// Command executor that delegates to an MCP tool.
pub struct McpCommandExecutor<T: McpTransport> {
    tool_name: String,
    client: Mutex<McpClient<T>>,
}

impl<T: McpTransport> McpCommandExecutor<T> {
    #[must_use]
    pub fn new(tool_name: String, client: McpClient<T>) -> Self {
        Self {
            tool_name,
            client: Mutex::new(client),
        }
    }
}

impl<T: McpTransport + Send> CommandExecutor for McpCommandExecutor<T> {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        let arguments = input
            .into_iter()
            .next()
            .map(|f| f.payload)
            .unwrap_or_else(|| json!({}));

        let mut client = self.client.lock().map_err(|_e| PipelineError {
            retryable: false,
            failed_step_id: step.step_id.clone(),
            error_kind: ErrorKind::ExecutionFailed,
        })?;

        let result = client
            .call_tool(&self.tool_name, arguments)
            .map_err(|_| PipelineError {
                retryable: false,
                failed_step_id: step.step_id.clone(),
                error_kind: ErrorKind::ExecutionFailed,
            })?;

        let payload = json!({
            "content": result.content,
            "is_error": result.is_error,
            "_taint": "untrusted_content",
            "_source": "mcp",
        });

        let frame = FrameEnvelope {
            seq: 0,
            step_id: step.step_id.clone(),
            frame_type: FrameType::JsonValue,
            payload,
        };

        Ok(vec![frame])
    }
}
