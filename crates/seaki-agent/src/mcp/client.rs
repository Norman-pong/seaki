//! Synchronous MCP client over any `McpTransport`.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

use super::protocol::{
    JsonRpcRequest, JsonRpcResponse, McpContent, McpError, McpResource, McpServerInfo, McpTool,
    McpToolResult,
};
use super::transport::McpTransport;

/// MCP client that wraps a transport and manages request IDs.
pub struct McpClient<T: McpTransport> {
    transport: T,
    next_id: u64,
}

impl<T: McpTransport> McpClient<T> {
    /// Create a new client with the given transport.
    #[must_use]
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            next_id: 1,
        }
    }

    /// Send an `initialize` request and return server info.
    ///
    /// # Errors
    /// Returns `McpError` on transport, protocol, or JSON-RPC errors.
    pub fn initialize(&mut self) -> Result<McpServerInfo, McpError> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "seaki", "version": env!("CARGO_PKG_VERSION", "0.1.0") }
        });
        let result: Value = self.send_request("initialize", params)?;
        let server_info = result
            .get("serverInfo")
            .ok_or_else(|| McpError::Protocol("missing serverInfo".to_string()))?;
        serde_json::from_value(server_info.clone())
            .map_err(|e| McpError::Protocol(format!("invalid serverInfo: {e}")))
    }

    /// List available tools from the server.
    ///
    /// # Errors
    /// Returns `McpError` on transport, protocol, or JSON-RPC errors.
    pub fn list_tools(&mut self) -> Result<Vec<McpTool>, McpError> {
        let params: Value = json!({});
        let result: Value = self.send_request("tools/list", params)?;
        let tools = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| McpError::Protocol("missing tools array".to_string()))?;
        tools
            .iter()
            .map(|t| {
                serde_json::from_value(t.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid tool: {e}")))
            })
            .collect()
    }

    /// Call a tool by name with the given arguments.
    ///
    /// # Errors
    /// Returns `McpError` on transport, protocol, or JSON-RPC errors.
    /// Tool-level errors are indicated by `McpToolResult::is_error`.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<McpToolResult, McpError> {
        let params = json!({
            "name": name,
            "arguments": arguments,
        });
        let result: Value = self
            .send_request("tools/call", params)
            .map_err(|e| match e {
                McpError::JsonRpc(ref err) if err.code == -32601 => {
                    McpError::ToolNotFound(name.to_string())
                }
                _ => e,
            })?;
        serde_json::from_value(result)
            .map_err(|e| McpError::Protocol(format!("invalid tool result: {e}")))
    }

    /// List available resources from the server.
    ///
    /// # Errors
    /// Returns `McpError` on transport, protocol, or JSON-RPC errors.
    pub fn list_resources(&mut self) -> Result<Vec<McpResource>, McpError> {
        let params: Value = json!({});
        let result: Value = self.send_request("resources/list", params)?;
        let resources = result
            .get("resources")
            .and_then(Value::as_array)
            .ok_or_else(|| McpError::Protocol("missing resources array".to_string()))?;
        resources
            .iter()
            .map(|r| {
                serde_json::from_value(r.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid resource: {e}")))
            })
            .collect()
    }

    /// Read a resource by URI.
    ///
    /// # Errors
    /// Returns `McpError` on transport, protocol, or JSON-RPC errors.
    pub fn read_resource(&mut self, uri: &str) -> Result<McpContent, McpError> {
        let params = json!({ "uri": uri });
        let result: Value = self.send_request("resources/read", params)?;
        serde_json::from_value(result)
            .map_err(|e| McpError::Protocol(format!("invalid resource content: {e}")))
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn send_request<Req, Res>(&mut self, method: &str, params: Req) -> Result<Res, McpError>
    where
        Req: Serialize,
        Res: DeserializeOwned,
    {
        let id = self.next_id();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };
        let request_json =
            serde_json::to_string(&request).map_err(|e| McpError::Protocol(e.to_string()))?;
        let response_json = self.transport.send(&request_json)?;
        let response: JsonRpcResponse<Res> = serde_json::from_str(&response_json)
            .map_err(|e| McpError::InvalidResponse(format!("{e}: {response_json}")))?;

        if let Some(err) = response.error {
            return Err(McpError::JsonRpc(err));
        }

        response
            .result
            .ok_or_else(|| McpError::InvalidResponse("missing result and error".to_string()))
    }
}
