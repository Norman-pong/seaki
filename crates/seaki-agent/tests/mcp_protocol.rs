use seaki_agent::mcp::{
    JsonRpcError, JsonRpcRequest, McpContent, McpError, McpTool, McpToolResult,
};
use serde_json::json;

#[test]
fn mcp_tool_serde_roundtrip() {
    let tool = McpTool {
        name: "read_file".to_string(),
        description: Some("Read a file".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }),
    };
    let json_str = serde_json::to_string(&tool).unwrap();
    let decoded: McpTool = serde_json::from_str(&json_str).unwrap();
    assert_eq!(tool, decoded);
}

#[test]
fn mcp_content_text() {
    let content = McpContent::Text {
        text: "hello".to_string(),
    };
    let json_str = serde_json::to_string(&content).unwrap();
    assert!(json_str.contains("hello"));
    let decoded: McpContent = serde_json::from_str(&json_str).unwrap();
    assert_eq!(content, decoded);
}

#[test]
fn mcp_content_image() {
    let content = McpContent::Image {
        data: "base64data".to_string(),
        mime_type: "image/png".to_string(),
    };
    let json_str = serde_json::to_string(&content).unwrap();
    assert!(json_str.contains("base64data"));
    let decoded: McpContent = serde_json::from_str(&json_str).unwrap();
    assert_eq!(content, decoded);
}

#[test]
fn mcp_tool_result_serde() {
    let result = McpToolResult {
        content: vec![McpContent::Text {
            text: "result".to_string(),
        }],
        is_error: false,
    };
    let json_str = serde_json::to_string(&result).unwrap();
    let decoded: McpToolResult = serde_json::from_str(&json_str).unwrap();
    assert_eq!(result, decoded);
}

#[test]
fn json_rpc_request_serde() {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "tools/list".to_string(),
        params: json!({}),
    };
    let json_str = serde_json::to_string(&req).unwrap();
    assert!(json_str.contains("tools/list"));
    let decoded: JsonRpcRequest<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
    assert_eq!(req.id, decoded.id);
    assert_eq!(req.method, decoded.method);
}

#[test]
fn json_rpc_error_serde() {
    let err = JsonRpcError {
        code: -32601,
        message: "Method not found".to_string(),
        data: None,
    };
    let json_str = serde_json::to_string(&err).unwrap();
    let decoded: JsonRpcError = serde_json::from_str(&json_str).unwrap();
    assert_eq!(err, decoded);
}

#[test]
fn mcp_error_display() {
    let err = McpError::Transport("connection refused".to_string());
    assert_eq!(err.to_string(), "MCP transport error: connection refused");

    let err = McpError::ToolNotFound("foo".to_string());
    assert_eq!(err.to_string(), "MCP tool not found: foo");

    let err = McpError::UnsupportedTransport;
    assert_eq!(err.to_string(), "unsupported MCP transport");

    let err = McpError::JsonRpc(JsonRpcError {
        code: -32700,
        message: "Parse error".to_string(),
        data: None,
    });
    assert!(err.to_string().contains("Parse error"));
}
