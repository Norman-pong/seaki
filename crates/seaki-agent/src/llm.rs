use serde::{Deserialize, Serialize};

/// Role of a message in the LLM conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in the LLM conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: MessageRole,
    pub content: String,
    pub name: Option<String>, // for tool messages
}

/// Request to the LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

/// Response from the LLM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Error from the LLM client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    RequestFailed(String),
    ParseFailed(String),
    RateLimited { retry_after_ms: u64 },
    ModelUnavailable(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::RequestFailed(msg) => write!(f, "LLM request failed: {msg}"),
            LlmError::ParseFailed(msg) => write!(f, "LLM response parse failed: {msg}"),
            LlmError::RateLimited { retry_after_ms } => {
                write!(f, "LLM rate limited, retry after {retry_after_ms}ms")
            }
            LlmError::ModelUnavailable(model) => write!(f, "LLM model unavailable: {model}"),
        }
    }
}

impl std::error::Error for LlmError {}

/// Abstract LLM client.
pub trait LlmClient: Send + Sync {
    fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
}

/// Mock LLM client for testing.
pub struct MockLlmClient {
    pub fixed_response: Option<String>,
    pub call_count: std::sync::Mutex<usize>,
}

impl MockLlmClient {
    pub fn new() -> Self {
        Self {
            fixed_response: None,
            call_count: std::sync::Mutex::new(0),
        }
    }

    pub fn with_fixed_response(response: String) -> Self {
        Self {
            fixed_response: Some(response),
            call_count: std::sync::Mutex::new(0),
        }
    }
}

impl Default for MockLlmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmClient for MockLlmClient {
    fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        drop(count);

        let content = if let Some(ref fixed) = self.fixed_response {
            fixed.clone()
        } else {
            // Echo the last user message, or fall back to empty string.
            request
                .messages
                .iter()
                .rev()
                .find(|m| m.role == MessageRole::User)
                .map(|m| m.content.clone())
                .unwrap_or_default()
        };

        let prompt_tokens = request
            .messages
            .iter()
            .map(|m| m.content.len() as u32 / 4 + 1)
            .sum();
        let completion_tokens = content.len() as u32 / 4 + 1;

        Ok(LlmResponse {
            content,
            model: request.model,
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
            finish_reason: "stop".to_string(),
        })
    }
}

/// OpenAI-compatible LLM client (skeleton — HTTP not yet implemented).
pub struct OpenAiClient {
    pub api_base: String,
    pub api_key: String,
    pub default_model: String,
}

impl OpenAiClient {
    pub fn new(api_base: String, api_key: String, default_model: String) -> Self {
        Self {
            api_base,
            api_key,
            default_model,
        }
    }
}

impl LlmClient for OpenAiClient {
    fn complete(&self, _request: LlmRequest) -> Result<LlmResponse, LlmError> {
        Err(LlmError::ModelUnavailable(
            "HTTP not yet implemented".to_string(),
        ))
    }
}
