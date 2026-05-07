//! MCP protocol types: Tool, Resource, Content, JSON-RPC envelope, errors.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// MCP Types
// ---------------------------------------------------------------------------

/// An MCP tool definition returned by `tools/list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// An MCP resource definition returned by `resources/list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// A single content item inside an MCP tool result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpContent {
    Text { text: String },
    Image { data: String, mime_type: String },
    EmbeddedResource { resource: McpResource },
}

/// Result of calling an MCP tool via `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    #[serde(default)]
    pub is_error: bool,
}

/// Information about the MCP server returned by `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

// ---------------------------------------------------------------------------
// JSON-RPC envelope
// ---------------------------------------------------------------------------

/// A generic JSON-RPC request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: T,
}

/// A generic JSON-RPC response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ---------------------------------------------------------------------------
// MCP Error
// ---------------------------------------------------------------------------

/// Errors that can occur when interacting with an MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpError {
    Transport(String),
    Protocol(String),
    ToolNotFound(String),
    JsonRpc(JsonRpcError),
    Io(String),
    InvalidResponse(String),
    UnsupportedTransport,
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(msg) => write!(f, "MCP transport error: {msg}"),
            Self::Protocol(msg) => write!(f, "MCP protocol error: {msg}"),
            Self::ToolNotFound(name) => write!(f, "MCP tool not found: {name}"),
            Self::JsonRpc(err) => write!(f, "JSON-RPC error {}: {}", err.code, err.message),
            Self::Io(msg) => write!(f, "MCP I/O error: {msg}"),
            Self::InvalidResponse(msg) => write!(f, "MCP invalid response: {msg}"),
            Self::UnsupportedTransport => write!(f, "unsupported MCP transport"),
        }
    }
}

impl std::error::Error for McpError {}
