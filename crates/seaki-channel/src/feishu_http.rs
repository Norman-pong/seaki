//! Feishu HTTP ProviderDriver implementation.
//!
//! Replaces `FakeProviderDriver` with real Feishu/Lark API HTTP calls.
//! Uses `tokio::runtime::Runtime` + `block_on` to keep the public API
//! synchronous (matching the `ProviderDriver` trait).

use std::sync::Mutex;

use serde::Deserialize;

use crate::outbox::{OutboxItem, ProviderDriver, ProviderError, ProviderQueryResult};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for connecting to the Feishu/Lark API.
#[derive(Debug, Clone)]
pub struct FeishuProviderConfig {
    /// Feishu App ID.
    pub app_id: String,
    /// Feishu App Secret.
    pub app_secret: String,
    /// API base URL (default: `https://open.feishu.cn/open-apis`).
    pub api_base: String,
    /// HTTP request timeout in seconds (default: 30).
    pub timeout_secs: u64,
}

impl FeishuProviderConfig {
    /// Build configuration from environment variables.
    ///
    /// Reads `SEAKI_FEISHU_APP_ID` and `SEAKI_FEISHU_APP_SECRET`.
    /// Returns `None` if either variable is missing or empty.
    pub fn from_env() -> Option<Self> {
        let app_id = std::env::var("SEAKI_FEISHU_APP_ID").ok()?;
        let app_secret = std::env::var("SEAKI_FEISHU_APP_SECRET").ok()?;
        if app_id.is_empty() || app_secret.is_empty() {
            return None;
        }
        Some(Self {
            app_id,
            app_secret,
            api_base: "https://open.feishu.cn/open-apis".to_string(),
            timeout_secs: 30,
        })
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors specific to the Feishu HTTP transport layer.
#[derive(Debug)]
pub enum FeishuHttpError {
    /// Failed to obtain a tenant access token.
    TokenFailed(String),
    /// HTTP request failed (network / transport).
    RequestFailed(String),
    /// Failed to parse the Feishu API response.
    ResponseParseFailed(String),
    /// Rate limited by the Feishu API.
    RateLimited,
}

impl std::fmt::Display for FeishuHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TokenFailed(msg) => write!(f, "token failed: {msg}"),
            Self::RequestFailed(msg) => write!(f, "request failed: {msg}"),
            Self::ResponseParseFailed(msg) => write!(f, "response parse failed: {msg}"),
            Self::RateLimited => write!(f, "rate limited"),
        }
    }
}

impl std::error::Error for FeishuHttpError {}

// ---------------------------------------------------------------------------
// Token cache
// ---------------------------------------------------------------------------

/// A cached tenant access token with its expiry time.
struct CachedToken {
    access_token: String,
    /// When the token expires (monotonic clock).
    expires_at: std::time::Instant,
}

/// Thread-safe access token manager with lazy refresh.
struct FeishuAccessToken {
    token: Mutex<Option<CachedToken>>,
}

impl FeishuAccessToken {
    fn new() -> Self {
        Self {
            token: Mutex::new(None),
        }
    }

    /// Obtain a valid tenant access token, refreshing if necessary.
    ///
    /// Uses a 60-second buffer before actual expiry to avoid edge-case failures.
    async fn get(
        &self,
        config: &FeishuProviderConfig,
        client: &reqwest::Client,
    ) -> Result<String, FeishuHttpError> {
        // Fast path: check cache under lock
        {
            let guard = self.token.lock().unwrap();
            if let Some(ref cached) = *guard {
                if cached.expires_at > std::time::Instant::now() + std::time::Duration::from_secs(60)
                {
                    return Ok(cached.access_token.clone());
                }
            }
        }

        // Slow path: refresh token
        let url = format!("{}/auth/v3/tenant_access_token/internal", config.api_base);
        let body = serde_json::json!({
            "app_id": config.app_id,
            "app_secret": config.app_secret,
        });

        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| FeishuHttpError::TokenFailed(e.to_string()))?;

        let data: TokenResponse = resp
            .json()
            .await
            .map_err(|e| FeishuHttpError::ResponseParseFailed(e.to_string()))?;

        if data.code != 0 {
            return Err(FeishuHttpError::TokenFailed(format!(
                "code {}: {}",
                data.code, data.msg
            )));
        }

        let token = CachedToken {
            access_token: data.tenant_access_token.clone(),
            expires_at: std::time::Instant::now()
                + std::time::Duration::from_secs(data.expire),
        };

        let access_token = token.access_token.clone();
        *self.token.lock().unwrap() = Some(token);
        Ok(access_token)
    }

    /// Invalidate the cached token (e.g. after receiving a token-expired error).
    fn invalidate(&self) {
        *self.token.lock().unwrap() = None;
    }
}

/// Response from the Feishu tenant access token endpoint.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    code: i64,
    msg: String,
    tenant_access_token: String,
    expire: u64,
}

// ---------------------------------------------------------------------------
// Send-message response
// ---------------------------------------------------------------------------

/// Response from the Feishu send-message endpoint.
#[derive(Debug, Deserialize)]
struct SendMessageResponse {
    code: i64,
    msg: String,
    #[allow(dead_code)]
    data: Option<SendMessageData>,
}

#[derive(Debug, Deserialize)]
struct SendMessageData {
    #[allow(dead_code)]
    message_id: Option<String>,
}

// ---------------------------------------------------------------------------
// ProviderDriver
// ---------------------------------------------------------------------------

/// Real Feishu HTTP provider driver.
///
/// Wraps async HTTP calls behind a synchronous `ProviderDriver` interface
/// using an internal `tokio` runtime.
pub struct FeishuProviderDriver {
    config: FeishuProviderConfig,
    token: FeishuAccessToken,
    runtime: tokio::runtime::Runtime,
    client: reqwest::Client,
}

impl FeishuProviderDriver {
    /// Create a new driver from the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the tokio runtime cannot be created.
    pub fn new(config: FeishuProviderConfig) -> Result<Self, String> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            config,
            token: FeishuAccessToken::new(),
            runtime,
            client,
        })
    }

    /// Create a new driver from environment variables.
    ///
    /// Returns `None` if `SEAKI_FEISHU_APP_ID` or `SEAKI_FEISHU_APP_SECRET`
    /// are not set.
    pub fn from_env() -> Option<Self> {
        let config = FeishuProviderConfig::from_env()?;
        Self::new(config).ok()
    }

    /// Async send implementation.
    async fn send_async(&self, item: &OutboxItem) -> Result<(), ProviderError> {
        let token = self
            .token
            .get(&self.config, &self.client)
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let result = self.do_send(&token, &item.payload).await;

        match result {
            Ok(()) => Ok(()),
            Err(ProviderError::Rejected(msg)) if msg.contains("99991400") => {
                // Token expired — invalidate and retry once
                self.token.invalidate();
                let new_token = self
                    .token
                    .get(&self.config, &self.client)
                    .await
                    .map_err(|e| ProviderError::Network(e.to_string()))?;
                self.do_send(&new_token, &item.payload).await
            }
            other => other,
        }
    }

    /// Execute the POST /im/v1/messages call.
    async fn do_send(&self, token: &str, payload: &str) -> Result<(), ProviderError> {
        // Parse payload to extract receive_id_type and build the request body.
        // The payload JSON was built by FeishuChannelAdapter::build_outbound and
        // contains: receive_id, receive_id_type, msg_type, content, uuid,
        // reply_in_thread, root_id.
        let payload_value: serde_json::Value = serde_json::from_str(payload)
            .map_err(|e| ProviderError::Rejected(format!("invalid payload json: {e}")))?;

        let receive_id_type = payload_value
            .get("receive_id_type")
            .and_then(|v| v.as_str())
            .unwrap_or("chat_id");

        let url = format!(
            "{}/im/v1/messages?receive_id_type={receive_id_type}",
            self.config.api_base
        );

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .json(&payload_value)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }

        let data: SendMessageResponse = serde_json::from_str(&body)
            .map_err(|e| ProviderError::Rejected(format!("response parse failed: {e}")))?;

        if data.code == 0 {
            Ok(())
        } else if data.code == 99991401 {
            Err(ProviderError::RateLimited)
        } else {
            Err(ProviderError::Rejected(format!(
                "feishu code {}: {}",
                data.code, data.msg
            )))
        }
    }

    /// Async idempotency query (not directly supported by Feishu).
    async fn query_async(&self, _key: &str) -> ProviderQueryResult {
        // Feishu does not provide a direct idempotency query API.
        // Return NotFound so the dispatcher treats the item as needing a send.
        ProviderQueryResult::NotFound
    }
}

impl std::fmt::Debug for FeishuProviderDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeishuProviderDriver")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ProviderDriver for FeishuProviderDriver {
    fn send(&self, item: &OutboxItem) -> Result<(), ProviderError> {
        self.runtime.block_on(self.send_async(item))
    }

    fn query_idempotency(&self, key: &str) -> ProviderQueryResult {
        self.runtime.block_on(self.query_async(key))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    /// Global lock to serialize tests that mutate environment variables.
    static ENV_LOCK: LazyLock<std::sync::Mutex<()>> = LazyLock::new(|| std::sync::Mutex::new(()));

    #[test]
    fn config_from_env_without_vars_returns_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("SEAKI_FEISHU_APP_ID");
        std::env::remove_var("SEAKI_FEISHU_APP_SECRET");
        assert!(FeishuProviderConfig::from_env().is_none());
    }

    #[test]
    fn config_from_env_with_vars_returns_some() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEAKI_FEISHU_APP_ID", "test_app_id");
        std::env::set_var("SEAKI_FEISHU_APP_SECRET", "test_app_secret");

        let config = FeishuProviderConfig::from_env();
        assert!(config.is_some());

        let config = config.unwrap();
        assert_eq!(config.app_id, "test_app_id");
        assert_eq!(config.app_secret, "test_app_secret");
        assert_eq!(
            config.api_base,
            "https://open.feishu.cn/open-apis"
        );
        assert_eq!(config.timeout_secs, 30);

        // Clean up
        std::env::remove_var("SEAKI_FEISHU_APP_ID");
        std::env::remove_var("SEAKI_FEISHU_APP_SECRET");
    }

    #[test]
    fn config_from_env_with_empty_vars_returns_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("SEAKI_FEISHU_APP_ID", "");
        std::env::set_var("SEAKI_FEISHU_APP_SECRET", "secret");

        assert!(FeishuProviderConfig::from_env().is_none());

        std::env::remove_var("SEAKI_FEISHU_APP_ID");
        std::env::remove_var("SEAKI_FEISHU_APP_SECRET");
    }

    #[test]
    fn feishu_send_request_serialization() {
        // Build a payload matching what FeishuChannelAdapter::build_outbound produces.
        let payload = serde_json::json!({
            "receive_id": "oc_abcdef123456",
            "receive_id_type": "chat_id",
            "msg_type": "text",
            "content": "{\"text\":\"hello world\"}",
            "uuid": "idem-key-001",
            "reply_in_thread": false,
            "root_id": null,
        });

        let payload_str = serde_json::to_string(&payload).unwrap();

        // Verify round-trip
        let parsed: serde_json::Value = serde_json::from_str(&payload_str).unwrap();
        assert_eq!(parsed["receive_id"], "oc_abcdef123456");
        assert_eq!(parsed["receive_id_type"], "chat_id");
        assert_eq!(parsed["msg_type"], "text");
        assert_eq!(parsed["uuid"], "idem-key-001");
        assert_eq!(parsed["reply_in_thread"], false);
    }

    #[test]
    fn driver_new_creates_successfully() {
        let config = FeishuProviderConfig {
            app_id: "cli_test".to_string(),
            app_secret: "secret_test".to_string(),
            api_base: "https://open.feishu.cn/open-apis".to_string(),
            timeout_secs: 5,
        };
        let driver = FeishuProviderDriver::new(config);
        assert!(driver.is_ok());
    }

    #[test]
    fn driver_from_env_returns_none_when_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("SEAKI_FEISHU_APP_ID");
        std::env::remove_var("SEAKI_FEISHU_APP_SECRET");
        assert!(FeishuProviderDriver::from_env().is_none());
    }

    /// Integration test: obtain a real tenant access token.
    /// Requires SEAKI_FEISHU_APP_ID and SEAKI_FEISHU_APP_SECRET to be set
    /// to valid Feishu test app credentials.
    #[tokio::test]
    #[ignore]
    async fn feishu_gets_tenant_token() {
        let config = FeishuProviderConfig::from_env().expect("feishu env vars not set");
        let client = reqwest::Client::new();
        let token_mgr = FeishuAccessToken::new();

        let token = token_mgr.get(&config, &client).await;
        assert!(token.is_ok(), "token request failed: {:?}", token.err());
        let token = token.unwrap();
        assert!(!token.is_empty());
    }

    /// Integration test: send a message to a test group.
    /// Requires environment variables and a valid test chat ID in payload.
    #[tokio::test]
    #[ignore]
    async fn feishu_sends_message_to_test_group() {
        let config = FeishuProviderConfig::from_env().expect("feishu env vars not set");
        let client = reqwest::Client::new();
        let token_mgr = FeishuAccessToken::new();

        let token = token_mgr
            .get(&config, &client)
            .await
            .expect("failed to get token");

        let payload = serde_json::json!({
            "receive_id": "oc_test_chat_id",
            "receive_id_type": "chat_id",
            "msg_type": "text",
            "content": "{\"text\":\"integration test from seaki\"}",
            "uuid": format!("test-{}", uuid::Uuid::now_v7()),
        });

        let url = format!(
            "{}/im/v1/messages?receive_id_type=chat_id",
            config.api_base
        );
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .json(&payload)
            .send()
            .await;

        assert!(resp.is_ok(), "request failed: {:?}", resp.err());
        let resp = resp.unwrap();
        let body: serde_json::Value = resp.json().await.expect("failed to parse response");
        assert_eq!(body["code"], 0, "feishu returned error: {body}");
    }
}
