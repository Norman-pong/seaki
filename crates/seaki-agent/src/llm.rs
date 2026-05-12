use serde::{Deserialize, Serialize};

use crate::runtime_handle::AgentRuntimeHandle;

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

// ---------------------------------------------------------------------------
// OpenAiClient — real implementation
// ---------------------------------------------------------------------------

/// Configuration for [`OpenAiClient`].
#[derive(Debug, Clone)]
pub struct OpenAiClientConfig {
    pub api_base: String,
    pub api_key: String,
    pub default_model: String,
    pub timeout_secs: u64,
}

impl OpenAiClientConfig {
    /// Reads configuration from environment variables:
    ///
    /// - `SEAKI_LLM_API_BASE` (defaults to `https://api.openai.com/v1`)
    /// - `SEAKI_LLM_API_KEY`  (required — returns `None` if missing)
    /// - `SEAKI_LLM_MODEL`    (defaults to `gpt-4o-mini`)
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("SEAKI_LLM_API_KEY").ok();
        if api_key.as_ref().is_none_or(|k| k.is_empty()) {
            return None;
        }
        Some(Self {
            api_base: std::env::var("SEAKI_LLM_API_BASE")
                .ok()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            api_key: api_key.unwrap(),
            default_model: std::env::var("SEAKI_LLM_MODEL")
                .ok()
                .unwrap_or_else(|| "gpt-4o-mini".to_string()),
            timeout_secs: 120,
        })
    }
}

/// OpenAI-compatible LLM client backed by `async-openai`.
///
/// Uses [`AgentRuntimeHandle::block_on`] to bridge the synchronous
/// [`LlmClient::complete`] call to the async `async-openai` API.
pub struct OpenAiClient {
    config: OpenAiClientConfig,
    runtime_handle: AgentRuntimeHandle,
}

impl OpenAiClient {
    pub fn new(config: OpenAiClientConfig, runtime_handle: AgentRuntimeHandle) -> Self {
        Self {
            config,
            runtime_handle,
        }
    }

    /// Convenience constructor that creates a default [`AgentRuntimeHandle`].
    pub fn with_default_runtime(config: OpenAiClientConfig) -> Self {
        Self::new(config, AgentRuntimeHandle::new())
    }
}

impl LlmClient for OpenAiClient {
    fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let openai_req = convert_request(&request, &self.config.default_model)?;

        let config = async_openai::config::OpenAIConfig::new()
            .with_api_base(&self.config.api_base)
            .with_api_key(&self.config.api_key);
        let client = async_openai::Client::with_config(config);

        self.runtime_handle.block_on(async {
            let resp = client.chat().create(openai_req).await;
            match resp {
                Ok(r) => convert_response(&r),
                Err(e) => Err(map_openai_error(&e)),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Convert an [`LlmRequest`] into a `CreateChatCompletionRequest`.
fn convert_request(
    req: &LlmRequest,
    fallback_model: &str,
) -> Result<async_openai::types::chat::CreateChatCompletionRequest, LlmError> {
    use async_openai::types::chat::{
        ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestToolMessage,
        ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessage,
    };

    let model = if req.model.is_empty() {
        fallback_model.to_string()
    } else {
        req.model.clone()
    };

    let messages: Vec<ChatCompletionRequestMessage> = req
        .messages
        .iter()
        .map(|m| match m.role {
            MessageRole::System => {
                ChatCompletionRequestSystemMessage::from(m.content.clone()).into()
            }
            MessageRole::User => {
                ChatCompletionRequestUserMessage::from(m.content.clone()).into()
            }
            MessageRole::Assistant => {
                ChatCompletionRequestAssistantMessage::from(m.content.clone()).into()
            }
            MessageRole::Tool => {
                let tool_id = m.name.clone().unwrap_or_else(|| "unknown".to_string());
                ChatCompletionRequestToolMessage {
                    content: ChatCompletionRequestToolMessageContent::from(m.content.clone()),
                    tool_call_id: tool_id,
                }
                .into()
            }
        })
        .collect();

    let mut binding =
        async_openai::types::chat::CreateChatCompletionRequestArgs::default();
    binding.model(model).messages(messages);

    if let Some(temp) = req.temperature {
        binding.temperature(temp);
    }
    if let Some(max_tokens) = req.max_tokens {
        binding.max_tokens(max_tokens);
    }

    binding
        .build()
        .map_err(|e| LlmError::RequestFailed(format!("failed to build request: {e}")))
}

/// Convert an OpenAI API response into an [`LlmResponse`].
fn convert_response(resp: &async_openai::types::chat::CreateChatCompletionResponse) -> Result<LlmResponse, LlmError> {
    let choice = resp
        .choices
        .first()
        .ok_or_else(|| LlmError::ParseFailed("no choices in response".to_string()))?;

    let content = choice
        .message
        .content
        .clone()
        .unwrap_or_default();

    let finish_reason = choice
        .finish_reason
        .map(|fr| format!("{fr:?}").to_lowercase())
        .unwrap_or_else(|| "unknown".to_string());

    let usage = resp.usage.as_ref();
    let token_usage = if let Some(u) = usage {
        TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }
    } else {
        TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        }
    };

    Ok(LlmResponse {
        content,
        model: resp.model.clone(),
        usage: token_usage,
        finish_reason,
    })
}

/// Map an `async_openai` error to [`LlmError`].
fn map_openai_error(err: &async_openai::error::OpenAIError) -> LlmError {
    match err {
        async_openai::error::OpenAIError::ApiError(api_err) => {
            // OpenAI returns code "rate_limit_exceeded" for rate limits.
            if api_err
                .code
                .as_deref()
                .is_some_and(|c| c.contains("rate_limit"))
            {
                LlmError::RateLimited {
                    retry_after_ms: 60_000,
                }
            } else {
                LlmError::RequestFailed(err.to_string())
            }
        }
        _ => LlmError::RequestFailed(err.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- OpenAiClientConfig::from_env tests ---
    //
    // Environment variables are process-global, so we serialize env tests
    // with a shared mutex to avoid data races between parallel test threads.

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn openai_config_from_env_without_key_returns_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Ensure SEAKI_LLM_API_KEY is not set.
        std::env::remove_var("SEAKI_LLM_API_KEY");
        assert!(
            OpenAiClientConfig::from_env().is_none(),
            "from_env should return None when SEAKI_LLM_API_KEY is unset"
        );
    }

    #[test]
    fn openai_config_from_env_reads_vars() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEAKI_LLM_API_KEY", "test-key-123");
        std::env::set_var("SEAKI_LLM_API_BASE", "http://localhost:11434/v1");
        std::env::set_var("SEAKI_LLM_MODEL", "llama3");

        let config = OpenAiClientConfig::from_env().expect("from_env should return Some");
        assert_eq!(config.api_key, "test-key-123");
        assert_eq!(config.api_base, "http://localhost:11434/v1");
        assert_eq!(config.default_model, "llama3");
        assert_eq!(config.timeout_secs, 120);

        // Clean up.
        std::env::remove_var("SEAKI_LLM_API_KEY");
        std::env::remove_var("SEAKI_LLM_API_BASE");
        std::env::remove_var("SEAKI_LLM_MODEL");
    }

    // --- OpenAiClient::complete error handling ---

    #[test]
    fn openai_client_complete_returns_error_without_server() {
        // Point at a host that definitely has no server running.
        let config = OpenAiClientConfig {
            api_base: "http://127.0.0.1:1/v1".to_string(),
            api_key: "fake-key".to_string(),
            default_model: "gpt-4o-mini".to_string(),
            timeout_secs: 5,
        };
        let client = OpenAiClient::with_default_runtime(config);

        let request = LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                name: None,
            }],
            temperature: None,
            max_tokens: None,
        };

        let result = client.complete(request);
        assert!(
            result.is_err(),
            "complete should return an error when no server is reachable"
        );
        match result {
            Err(LlmError::RequestFailed(msg)) => {
                assert!(
                    !msg.is_empty(),
                    "error message should be non-empty"
                );
            }
            other => panic!("expected RequestFailed, got: {other:?}"),
        }
    }

    // --- Integration test (requires Ollama running locally) ---

    #[test]
    #[ignore]
    fn openai_client_integration_with_ollama() {
        let config = OpenAiClientConfig {
            api_base: "http://localhost:11434/v1".to_string(),
            api_key: "ollama".to_string(), // Ollama accepts any non-empty key
            default_model: "llama3".to_string(),
            timeout_secs: 120,
        };
        let client = OpenAiClient::with_default_runtime(config);

        let request = LlmRequest {
            model: "llama3".to_string(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "Say exactly: hello world".to_string(),
                name: None,
            }],
            temperature: Some(0.0),
            max_tokens: Some(32),
        };

        let response = client.complete(request).expect("Ollama request should succeed");
        assert!(!response.content.is_empty(), "response content should not be empty");
        assert!(
            response.content.to_lowercase().contains("hello"),
            "response should contain 'hello': got '{}'",
            response.content
        );
    }

    // --- convert_request / convert_response unit tests ---

    #[test]
    fn convert_request_uses_fallback_model_when_empty() {
        let req = LlmRequest {
            model: String::new(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "hi".to_string(),
                name: None,
            }],
            temperature: None,
            max_tokens: None,
        };
        let result = convert_request(&req, "fallback-model").unwrap();
        assert_eq!(result.model, "fallback-model");
    }

    #[test]
    fn convert_request_preserves_model_when_set() {
        let req = LlmRequest {
            model: "my-model".to_string(),
            messages: vec![LlmMessage {
                role: MessageRole::User,
                content: "hi".to_string(),
                name: None,
            }],
            temperature: Some(0.5),
            max_tokens: Some(100),
        };
        let result = convert_request(&req, "fallback").unwrap();
        assert_eq!(result.model, "my-model");
        assert_eq!(result.temperature, Some(0.5));
        #[allow(deprecated)]
        let max_tokens = result.max_tokens;
        assert_eq!(max_tokens, Some(100));
    }

    #[test]
    fn convert_request_maps_all_message_roles() {
        let req = LlmRequest {
            model: "test".to_string(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: "sys".to_string(),
                    name: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: "usr".to_string(),
                    name: None,
                },
                LlmMessage {
                    role: MessageRole::Assistant,
                    content: "ast".to_string(),
                    name: None,
                },
                LlmMessage {
                    role: MessageRole::Tool,
                    content: "tool".to_string(),
                    name: Some("my-tool".to_string()),
                },
            ],
            temperature: None,
            max_tokens: None,
        };
        let result = convert_request(&req, "fallback").unwrap();
        assert_eq!(result.messages.len(), 4);
    }
}
