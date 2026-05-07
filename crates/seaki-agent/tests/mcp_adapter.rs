use seaki_agent::mcp::{McpClient, McpCommandExecutor, McpToPipeAdapter, McpTransport};
use seaki_pipe::ast::{ComposedStep, FrameType};
use seaki_pipe::dry_run::ErrorKind;
use seaki_pipe::dry_run::FrameEnvelope;
use seaki_pipe::registry::{CommandRegistry, SideEffectLevel};
use seaki_pipe::run::{CommandExecutor, ExecutionContext};
use seaki_pipe::Cardinality;
use serde_json::json;

struct MockTransport {
    responses: Vec<String>,
}

impl MockTransport {
    fn new(responses: Vec<String>) -> Self {
        Self { responses }
    }
}

impl McpTransport for MockTransport {
    fn send(&mut self, _request: &str) -> Result<String, seaki_agent::mcp::McpError> {
        let resp = self.responses.remove(0);
        Ok(resp)
    }
}

fn make_test_step() -> ComposedStep {
    ComposedStep {
        step_id: "step-1".to_string(),
        command_id: "mcp:test:tool_a".to_string(),
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::JsonValue, Cardinality::One),
        input_binding: seaki_pipe::ast::InputBinding::PreviousStep,
        failure_policy: seaki_pipe::ast::FailurePolicy::FailFast,
        side_effect_level: SideEffectLevel::ExternalIrreversible,
        args: json!({}),
    }
}

fn empty_ctx() -> ExecutionContext {
    ExecutionContext {
        workspace_id: "w1".to_string(),
        actor_id: "a1".to_string(),
        pipeline_id: "p1".to_string(),
        audit: Vec::new(),
        resource_used: Default::default(),
    }
}

#[test]
fn adapter_registers_single_tool() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "tools": [
                { "name": "tool_a", "description": "desc a", "inputSchema": { "type": "object" } }
            ]
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let mut registry = CommandRegistry::new();
    let adapter = McpToPipeAdapter::new("test".to_string());
    let ids = adapter.register_tools(&mut client, &mut registry).unwrap();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0], "mcp:test:tool_a");
}

#[test]
fn adapter_registers_multiple_tools() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "tools": [
                { "name": "tool_a", "description": "a", "inputSchema": { "type": "object" } },
                { "name": "tool_b", "description": "b", "inputSchema": { "type": "object" } }
            ]
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let mut registry = CommandRegistry::new();
    let adapter = McpToPipeAdapter::new("test".to_string());
    let ids = adapter.register_tools(&mut client, &mut registry).unwrap();
    assert_eq!(ids.len(), 2);
}

#[test]
fn adapter_command_id_format() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "tools": [
                { "name": "my_tool", "description": "a", "inputSchema": { "type": "object" } }
            ]
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let mut registry = CommandRegistry::new();
    let adapter = McpToPipeAdapter::new("my_server".to_string());
    let ids = adapter.register_tools(&mut client, &mut registry).unwrap();
    assert_eq!(ids[0], "mcp:my_server:my_tool");
}

#[test]
fn adapter_side_effect_level_external() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "tools": [
                { "name": "tool_a", "description": "a", "inputSchema": { "type": "object" } }
            ]
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let mut registry = CommandRegistry::new();
    let adapter = McpToPipeAdapter::new("test".to_string());
    adapter.register_tools(&mut client, &mut registry).unwrap();
    let manifest = registry.inspect("mcp:test:tool_a").unwrap();
    assert_eq!(
        manifest.side_effect_level,
        SideEffectLevel::ExternalIrreversible
    );
}

#[test]
fn adapter_schema_conversion() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "tools": [
                { "name": "tool_a", "description": "a", "inputSchema": { "type": "object", "properties": { "x": { "type": "string" } } } }
            ]
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let mut registry = CommandRegistry::new();
    let adapter = McpToPipeAdapter::new("test".to_string());
    adapter.register_tools(&mut client, &mut registry).unwrap();
    let manifest = registry.inspect("mcp:test:tool_a").unwrap();
    assert_eq!(
        manifest.input_frame,
        (FrameType::JsonValue, Cardinality::One)
    );
    assert_eq!(
        manifest.output_frame,
        (FrameType::JsonValue, Cardinality::One)
    );
    assert!(manifest.output_schema.get("properties").is_some());
}

#[test]
fn mcp_executor_invokes_tool() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{ "type": "text", "text": "result" }],
            "is_error": false
        }
    })
    .to_string()]);
    let client = McpClient::new(transport);
    let executor = McpCommandExecutor::new("tool_a".to_string(), client);
    let step = make_test_step();
    let input = vec![FrameEnvelope {
        seq: 1,
        step_id: "step-1".to_string(),
        frame_type: FrameType::JsonValue,
        payload: json!({ "arg": 42 }),
    }];
    let mut ctx = empty_ctx();
    let output = executor.execute(&step, input, &mut ctx).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].frame_type, FrameType::JsonValue);
}

#[test]
fn mcp_executor_taint_untrusted() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{ "type": "text", "text": "result" }],
            "is_error": false
        }
    })
    .to_string()]);
    let client = McpClient::new(transport);
    let executor = McpCommandExecutor::new("tool_a".to_string(), client);
    let step = make_test_step();
    let input = vec![FrameEnvelope {
        seq: 1,
        step_id: "step-1".to_string(),
        frame_type: FrameType::JsonValue,
        payload: json!({}),
    }];
    let mut ctx = empty_ctx();
    let output = executor.execute(&step, input, &mut ctx).unwrap();
    let payload = &output[0].payload;
    assert_eq!(payload.get("_taint").unwrap(), "untrusted_content");
    assert_eq!(payload.get("_source").unwrap(), "mcp");
}

#[test]
fn mcp_executor_error_propagation() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": -32600, "message": "Invalid Request" }
    })
    .to_string()]);
    let client = McpClient::new(transport);
    let executor = McpCommandExecutor::new("tool_a".to_string(), client);
    let step = make_test_step();
    let input = vec![FrameEnvelope {
        seq: 1,
        step_id: "step-1".to_string(),
        frame_type: FrameType::JsonValue,
        payload: json!({}),
    }];
    let mut ctx = empty_ctx();
    let result = executor.execute(&step, input, &mut ctx);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err.error_kind, ErrorKind::ExecutionFailed));
}

#[test]
fn mcp_executor_empty_input() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{ "type": "text", "text": "ok" }],
            "is_error": false
        }
    })
    .to_string()]);
    let client = McpClient::new(transport);
    let executor = McpCommandExecutor::new("tool_a".to_string(), client);
    let step = make_test_step();
    let input: Vec<FrameEnvelope> = vec![];
    let mut ctx = empty_ctx();
    let output = executor.execute(&step, input, &mut ctx).unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(
        output[0]
            .payload
            .get("content")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
