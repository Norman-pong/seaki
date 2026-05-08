//! Feishu (Lark) channel protocol adapter.
//!
//! Provides inbound event parsing, webhook verification, and outbound
//! message construction for the Feishu/Lark messaging platform.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::fake_provider::{ChannelEvent, ChannelMessagePayload};
use crate::grant::ChannelAttachmentRef;
use crate::ingress::ResolvedIdentity;
use crate::webhook::{WebhookError, WebhookVerifier};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur when parsing or validating Feishu events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuParseError {
    InvalidJson(String),
    MissingField(&'static str),
    UnsupportedMessageType(String),
    InvalidSignature,
    TimestampExpired,
}

impl std::fmt::Display for FeishuParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(msg) => write!(f, "invalid json: {msg}"),
            Self::MissingField(field) => write!(f, "missing field: {field}"),
            Self::UnsupportedMessageType(ty) => write!(f, "unsupported message type: {ty}"),
            Self::InvalidSignature => write!(f, "invalid signature"),
            Self::TimestampExpired => write!(f, "timestamp expired"),
        }
    }
}

impl std::error::Error for FeishuParseError {}

/// High-level adapter errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuAdapterError {
    Parse(FeishuParseError),
    Webhook(WebhookError),
    IdentityNotResolved,
}

impl std::fmt::Display for FeishuAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse error: {e}"),
            Self::Webhook(e) => write!(f, "webhook error: {e}"),
            Self::IdentityNotResolved => write!(f, "identity not resolved"),
        }
    }
}

impl std::error::Error for FeishuAdapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(e) => Some(e),
            Self::Webhook(e) => Some(e),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Inbound event model
// ---------------------------------------------------------------------------

/// Top-level Feishu webhook event envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeishuEvent {
    pub header: FeishuEventHeader,
    pub event: FeishuEventBody,
}

/// Feishu event header metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeishuEventHeader {
    pub event_id: String,
    pub event_type: String,
    pub create_time: String,
    pub token: String,
    pub app_id: String,
    pub tenant_key: String,
}

/// Feishu event body containing sender and message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeishuEventBody {
    pub sender: FeishuSender,
    pub message: FeishuMessage,
}

/// Sender information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeishuSender {
    pub sender_id: FeishuUserId,
    pub sender_type: String,
    pub tenant_key: String,
}

/// Feishu user identifier triplet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeishuUserId {
    pub union_id: String,
    pub user_id: String,
    pub open_id: String,
}

/// Feishu message metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeishuMessage {
    pub message_id: String,
    pub chat_id: String,
    pub chat_type: String,
    pub msg_type: String,
    pub content: String,
    #[serde(default)]
    pub parent_message_id: Option<String>,
    pub create_time: String,
}

// ---------------------------------------------------------------------------
// Parsed message content
// ---------------------------------------------------------------------------

/// Parsed message content after extracting Feishu `content` JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedMessageContent {
    Text {
        text: String,
    },
    File {
        file_key: String,
        file_name: String,
        file_size: u64,
    },
    Image {
        image_key: String,
    },
}

/// Parse the `content` JSON string based on `msg_type`.
///
/// # Errors
///
/// Returns `FeishuParseError` if JSON is malformed, required fields are missing,
/// or the message type is unsupported.
pub fn parse_message_content(
    msg_type: &str,
    content_json: &str,
) -> Result<ParsedMessageContent, FeishuParseError> {
    let value: serde_json::Value = serde_json::from_str(content_json)
        .map_err(|e| FeishuParseError::InvalidJson(e.to_string()))?;

    match msg_type {
        "text" => {
            let text = value
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or(FeishuParseError::MissingField("text"))?
                .to_string();
            Ok(ParsedMessageContent::Text { text })
        }
        "file" => {
            let file_key = value
                .get("file_key")
                .and_then(|v| v.as_str())
                .ok_or(FeishuParseError::MissingField("file_key"))?
                .to_string();
            let file_name = value
                .get("file_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let file_size = value.get("file_size").and_then(|v| v.as_u64()).unwrap_or(0);
            Ok(ParsedMessageContent::File {
                file_key,
                file_name,
                file_size,
            })
        }
        "image" => {
            let image_key = value
                .get("image_key")
                .and_then(|v| v.as_str())
                .ok_or(FeishuParseError::MissingField("image_key"))?
                .to_string();
            Ok(ParsedMessageContent::Image { image_key })
        }
        other => Err(FeishuParseError::UnsupportedMessageType(other.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Webhook verifier
// ---------------------------------------------------------------------------

/// Feishu-specific webhook verifier.
///
/// Performs simplified verification suitable for M2-C05:
/// - `verification_token` comparison (the `signature` parameter is treated as the token)
/// - Timestamp tolerance (±5 minutes by default)
/// - Event-id deduplication (replay protection)
#[derive(Debug)]
pub struct FeishuWebhookVerifier {
    verification_token: String,
    #[allow(dead_code)]
    encrypt_key: Option<String>,
    seen_event_ids: Mutex<HashMap<String, SystemTime>>,
    ttl: Duration,
}

impl FeishuWebhookVerifier {
    /// Create a new verifier.
    #[must_use]
    pub fn new(verification_token: impl Into<String>, encrypt_key: Option<String>) -> Self {
        Self {
            verification_token: verification_token.into(),
            encrypt_key,
            seen_event_ids: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Set a custom TTL for timestamp and replay checks.
    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    fn evict_expired(&self) {
        let now = SystemTime::now();
        let mut seen = self.seen_event_ids.lock().unwrap();
        seen.retain(|_, &mut t| now.duration_since(t).unwrap_or(Duration::MAX) <= self.ttl);
    }
}

impl WebhookVerifier for FeishuWebhookVerifier {
    fn verify(
        &self,
        event_id: &str,
        _payload: &[u8],
        signature: &str,
        timestamp: SystemTime,
    ) -> Result<(), WebhookError> {
        self.evict_expired();

        // Replay check (first gate)
        {
            let seen = self.seen_event_ids.lock().unwrap();
            if seen.contains_key(event_id) {
                return Err(WebhookError::EventReplayed);
            }
        }

        // Timestamp check
        let now = SystemTime::now();
        if now.duration_since(timestamp).unwrap_or(Duration::MAX) > self.ttl {
            return Err(WebhookError::TimestampExpired);
        }

        // Simplified token verification (signature parameter treated as token)
        if signature != self.verification_token {
            return Err(WebhookError::SignatureMismatch);
        }

        // Replay check (second gate, after validation)
        let mut seen = self.seen_event_ids.lock().unwrap();
        if seen.contains_key(event_id) {
            return Err(WebhookError::EventReplayed);
        }
        seen.insert(event_id.to_string(), now);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Event parsing
// ---------------------------------------------------------------------------

/// Parse raw JSON bytes into a [`FeishuEvent`].
///
/// # Errors
///
/// Returns `FeishuParseError::InvalidJson` on malformed JSON.
pub fn parse_feishu_event(payload: &[u8]) -> Result<FeishuEvent, FeishuParseError> {
    serde_json::from_slice(payload).map_err(|e| FeishuParseError::InvalidJson(e.to_string()))
}

/// Convert a parsed Feishu event into a Seaki [`ChannelEvent`].
///
/// # Errors
///
/// Returns `FeishuParseError` if required fields are missing or content cannot be parsed.
pub fn feishu_event_to_channel_event(
    event: &FeishuEvent,
    resolved_identity: ResolvedIdentity,
) -> Result<ChannelEvent, FeishuParseError> {
    let msg = &event.event.message;
    let parsed = parse_message_content(&msg.msg_type, &msg.content)?;

    let (text, attachments) = match parsed {
        ParsedMessageContent::Text { text } => (text, Vec::new()),
        ParsedMessageContent::File {
            file_key,
            file_name,
            file_size,
        } => {
            let attachment = ChannelAttachmentRef {
                attachment_id: format!("feishu:{}:{}", event.header.event_id, file_key),
                provider: "feishu".to_string(),
                provider_tenant_id: event.header.tenant_key.clone(),
                provider_chat_id: msg.chat_id.clone(),
                provider_message_id: msg.message_id.clone(),
                provider_thread_id: msg.parent_message_id.clone().unwrap_or_default(),
                provider_file_key: file_key,
                provider_file_version: "1".to_string(),
                original_name: file_name,
                declared_mime: "application/octet-stream".to_string(),
                declared_size: file_size,
                content_hash: None,
                download_capability_required: true,
            };
            (String::new(), vec![attachment])
        }
        ParsedMessageContent::Image { image_key } => {
            let attachment = ChannelAttachmentRef {
                attachment_id: format!("feishu:{}:{}", event.header.event_id, image_key),
                provider: "feishu".to_string(),
                provider_tenant_id: event.header.tenant_key.clone(),
                provider_chat_id: msg.chat_id.clone(),
                provider_message_id: msg.message_id.clone(),
                provider_thread_id: msg.parent_message_id.clone().unwrap_or_default(),
                provider_file_key: image_key,
                provider_file_version: "1".to_string(),
                original_name: "image".to_string(),
                declared_mime: "image/*".to_string(),
                declared_size: 0,
                content_hash: None,
                download_capability_required: true,
            };
            (String::new(), vec![attachment])
        }
    };

    let channel_scope = format!(
        "workspace:{}/channel:{}/user:{}",
        resolved_identity.seaki_workspace_id, msg.chat_id, event.event.sender.sender_id.open_id
    );

    let now = SystemTime::now();

    Ok(ChannelEvent {
        event_id: event.header.event_id.clone(),
        event_type: "channel.message.received".to_string(),
        provider_tenant_id: event.header.tenant_key.clone(),
        channel_binding_id: msg.chat_id.clone(),
        provider_user_id: event.event.sender.sender_id.open_id.clone(),
        payload: ChannelMessagePayload { text, attachments },
        timestamp: now,
        seaki_workspace_id: resolved_identity.seaki_workspace_id,
        seaki_actor_id: resolved_identity.seaki_actor_id,
        workspace_role: resolved_identity.workspace_role,
        channel_scope,
        signature_verified_at: now,
        normalized_at: now,
    })
}

// ---------------------------------------------------------------------------
// Outbound message construction
// ---------------------------------------------------------------------------

/// Feishu send-message request payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeishuSendRequest {
    pub receive_id: String,
    pub receive_id_type: String,
    pub msg_type: String,
    pub content: String,
    pub uuid: String,
    pub reply_in_thread: bool,
}

/// Build a Feishu text reply payload JSON string.
#[must_use]
pub fn build_feishu_reply_payload(text: &str, parent_message_id: Option<&str>) -> String {
    if let Some(parent) = parent_message_id {
        serde_json::json!({
            "text": text,
            "reply_in_thread": true,
            "root_id": parent,
        })
        .to_string()
    } else {
        serde_json::json!({
            "text": text,
        })
        .to_string()
    }
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

/// Provenance metadata to append to outbound replies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeishuProvenance {
    pub transaction_id: String,
    pub source_id: String,
    pub wiki_patch_hash: Option<String>,
    pub citation_ids: Vec<String>,
    pub audit_id: String,
}

/// Format a reply text with provenance footer.
#[must_use]
pub fn format_reply_with_provenance(text: &str, provenance: &FeishuProvenance) -> String {
    let mut lines = vec![text.to_string()];
    lines.push(format!("\n— transaction: {}", provenance.transaction_id));
    lines.push(format!("source: {}", provenance.source_id));
    if let Some(ref hash) = provenance.wiki_patch_hash {
        lines.push(format!("wiki: {hash}"));
    }
    if !provenance.citation_ids.is_empty() {
        lines.push(format!("citations: {}", provenance.citation_ids.join(", ")));
    }
    lines.push(format!("audit: {}", provenance.audit_id));
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Adapter helpers
// ---------------------------------------------------------------------------

/// Stateless Feishu channel adapter helpers.
pub struct FeishuChannelAdapter;

impl FeishuChannelAdapter {
    /// Create a new adapter instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build an outbound Feishu send request from a grant and message text.
    ///
    /// # Errors
    ///
    /// Returns `FeishuAdapterError` if the grant scope cannot be parsed.
    pub fn build_outbound(
        &self,
        grant: &seaki_policy::grant::ChannelActionGrant,
        text: &str,
        provenance: &FeishuProvenance,
    ) -> Result<FeishuSendRequest, FeishuAdapterError> {
        let reply_text = format_reply_with_provenance(text, provenance);
        let content = serde_json::json!({ "text": reply_text }).to_string();

        // Parse scope "workspace:{ws}/channel:{chat_id}/user:{user_id}"
        // We need to extract chat_id from the scope.
        let receive_id = grant
            .scope
            .split('/')
            .find(|part| part.starts_with("channel:"))
            .map(|part| part.strip_prefix("channel:").unwrap_or(part))
            .ok_or(FeishuAdapterError::Parse(FeishuParseError::MissingField(
                "channel in scope",
            )))?
            .to_string();

        Ok(FeishuSendRequest {
            receive_id,
            receive_id_type: "chat_id".to_string(),
            msg_type: "text".to_string(),
            content,
            uuid: grant.idempotency_key.clone(),
            reply_in_thread: false,
        })
    }
}

impl Default for FeishuChannelAdapter {
    fn default() -> Self {
        Self::new()
    }
}
