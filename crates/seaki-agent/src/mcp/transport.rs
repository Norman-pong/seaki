//! MCP transport implementations: trait, stdio, and HTTP stub.

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

use super::protocol::McpError;

/// Synchronous transport for MCP JSON-RPC messages.
pub trait McpTransport {
    /// Send a JSON-RPC request string and return the raw response string.
    ///
    /// # Errors
    /// Returns `McpError::Transport` or `McpError::Io` on failure.
    fn send(&mut self, request: &str) -> Result<String, McpError>;
}

// ---------------------------------------------------------------------------
// StdioTransport
// ---------------------------------------------------------------------------

/// MCP transport over a child process's stdin/stdout.
pub struct StdioTransport {
    #[allow(dead_code)]
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    reader: std::io::BufReader<std::process::ChildStdout>,
}

impl StdioTransport {
    /// Spawn a new child process and wrap its stdio.
    ///
    /// # Errors
    /// Returns `McpError::Io` if the child process cannot be started.
    pub fn new(command: &str, args: &[&str]) -> Result<Self, McpError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| McpError::Io(format!("failed to spawn {command}: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Io("failed to open child stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Io("failed to open child stdout".to_string()))?;
        let reader = std::io::BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            reader,
        })
    }
}

impl McpTransport for StdioTransport {
    fn send(&mut self, request: &str) -> Result<String, McpError> {
        let line = format!("{request}\n");
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| McpError::Io(format!("stdin write failed: {e}")))?;
        self.stdin
            .flush()
            .map_err(|e| McpError::Io(format!("stdin flush failed: {e}")))?;

        let mut response = String::new();
        self.reader
            .read_line(&mut response)
            .map_err(|e| McpError::Io(format!("stdout read failed: {e}")))?;
        Ok(response)
    }
}

// ---------------------------------------------------------------------------
// HttpTransport (stub)
// ---------------------------------------------------------------------------

/// HTTP transport placeholder.
pub struct HttpTransport;

impl HttpTransport {
    /// Always returns `UnsupportedTransport`.
    ///
    /// # Errors
    /// Always returns `McpError::UnsupportedTransport`.
    pub fn new(_url: &str) -> Result<Self, McpError> {
        Err(McpError::UnsupportedTransport)
    }
}

impl McpTransport for HttpTransport {
    fn send(&mut self, _request: &str) -> Result<String, McpError> {
        Err(McpError::UnsupportedTransport)
    }
}
