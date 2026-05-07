use seaki_agent::mcp::{HttpTransport, McpError, McpTransport, StdioTransport};

/// Mock transport backed by a queue of response strings.
struct MockTransport {
    responses: Vec<String>,
    sent: Vec<String>,
}

impl MockTransport {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            sent: Vec::new(),
        }
    }
}

impl McpTransport for MockTransport {
    fn send(&mut self, request: &str) -> Result<String, McpError> {
        self.sent.push(request.to_string());
        let resp = self.responses.remove(0);
        Ok(resp)
    }
}

#[test]
fn mock_transport_send_receive() {
    let mut transport =
        MockTransport::new(vec![r#"{"jsonrpc":"2.0","id":1,"result":{}}"#.to_string()]);
    let resp = transport.send(r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#);
    assert!(resp.is_ok());
    assert_eq!(transport.sent.len(), 1);
}

#[test]
fn stdio_transport_spawn_failure() {
    // Use a non-existent binary to trigger spawn failure.
    let result = StdioTransport::new("/nonexistent_binary_12345", &[]);
    assert!(matches!(result, Err(McpError::Io(_))));
}

#[test]
fn http_transport_unsupported() {
    let result = HttpTransport::new("http://localhost:8080");
    assert!(matches!(result, Err(McpError::UnsupportedTransport)));
}
