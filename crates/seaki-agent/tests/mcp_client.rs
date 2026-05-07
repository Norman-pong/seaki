use seaki_agent::mcp::{JsonRpcError, McpClient, McpError, McpTransport};
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
    fn send(&mut self, _request: &str) -> Result<String, McpError> {
        let resp = self.responses.remove(0);
        Ok(resp)
    }
}

#[test]
fn client_initialize_success() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "test-server", "version": "1.0.0" },
            "capabilities": {}
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let info = client.initialize().unwrap();
    assert_eq!(info.name, "test-server");
    assert_eq!(info.version, "1.0.0");
}

#[test]
fn client_list_tools() {
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
    let tools = client.list_tools().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "tool_a");
}

#[test]
fn client_call_tool() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "content": [{ "type": "text", "text": "hello" }],
            "is_error": false
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let result = client.call_tool("tool_a", json!({})).unwrap();
    assert_eq!(result.content.len(), 1);
    assert!(!result.is_error);
}

#[test]
fn client_tool_not_found() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": -32601, "message": "Method not found" }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let result = client.call_tool("missing_tool", json!({}));
    assert!(matches!(result, Err(McpError::ToolNotFound(_))));
}

#[test]
fn client_json_rpc_error() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": -32600, "message": "Invalid Request" }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let result = client.list_tools();
    assert!(matches!(
        result,
        Err(McpError::JsonRpc(JsonRpcError { code: -32600, .. }))
    ));
}

#[test]
fn client_list_resources() {
    let transport = MockTransport::new(vec![json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "resources": [
                { "uri": "file:///tmp/a.txt", "name": "a.txt", "mimeType": "text/plain" }
            ]
        }
    })
    .to_string()]);
    let mut client = McpClient::new(transport);
    let resources = client.list_resources().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "file:///tmp/a.txt");
}
