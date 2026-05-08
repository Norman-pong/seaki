use std::time::{Duration, SystemTime};

use seaki_channel::feishu::{
    build_feishu_reply_payload, feishu_event_to_channel_event, format_reply_with_provenance,
    parse_feishu_event, parse_message_content, FeishuAdapterError, FeishuChannelAdapter,
    FeishuEvent, FeishuEventBody, FeishuEventHeader, FeishuMessage, FeishuParseError,
    FeishuProvenance, FeishuSender, FeishuUserId, FeishuWebhookVerifier, ParsedMessageContent,
};
use seaki_channel::ingress::ResolvedIdentity;
use seaki_channel::webhook::{WebhookError, WebhookVerifier};
use seaki_policy::grant::{ChannelActionGrant, Provenance};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sample_event_header() -> FeishuEventHeader {
    FeishuEventHeader {
        event_id: "evt-123".to_string(),
        event_type: "im.message.receive_v1".to_string(),
        create_time: "1234567890000".to_string(),
        token: "verif-token".to_string(),
        app_id: "app-xxx".to_string(),
        tenant_key: "tenant-1".to_string(),
    }
}

fn sample_sender() -> FeishuSender {
    FeishuSender {
        sender_id: FeishuUserId {
            union_id: "union-1".to_string(),
            user_id: "user-1".to_string(),
            open_id: "open-1".to_string(),
        },
        sender_type: "user".to_string(),
        tenant_key: "tenant-1".to_string(),
    }
}

fn sample_message_text() -> FeishuMessage {
    FeishuMessage {
        message_id: "msg-1".to_string(),
        chat_id: "chat-1".to_string(),
        chat_type: "p2p".to_string(),
        msg_type: "text".to_string(),
        content: r#"{"text":"hello feishu"}"#.to_string(),
        parent_message_id: None,
        create_time: "1234567890000".to_string(),
    }
}

fn sample_event() -> FeishuEvent {
    FeishuEvent {
        header: sample_event_header(),
        event: FeishuEventBody {
            sender: sample_sender(),
            message: sample_message_text(),
        },
    }
}

fn sample_event_json() -> String {
    serde_json::to_string(&sample_event()).unwrap()
}

fn resolved_identity() -> ResolvedIdentity {
    ResolvedIdentity {
        seaki_workspace_id: "ws-1".to_string(),
        seaki_actor_id: "actor-1".to_string(),
        workspace_role: "member".to_string(),
    }
}

fn sample_grant(scope: &str) -> ChannelActionGrant {
    ChannelActionGrant {
        grant_id: "grant-1".to_string(),
        scope: scope.to_string(),
        audience: "audience-1".to_string(),
        ttl: Duration::from_secs(300),
        uses_remaining: 1,
        idempotency_key: "idem-1".to_string(),
        allowed_actions: vec!["message.send".to_string()],
        provenance: Provenance {
            transaction_id: "tx-1".to_string(),
            source_id: "src-1".to_string(),
            citation_ids: vec!["c-1".to_string()],
            thread_scope: "thread-1".to_string(),
            audit_id: "audit-1".to_string(),
        },
        expires_at: SystemTime::now() + Duration::from_secs(300),
    }
}

// ---------------------------------------------------------------------------
// Verifier tests
// ---------------------------------------------------------------------------

#[test]
fn feishu_verifier_accepts_valid_signature() {
    let verifier = FeishuWebhookVerifier::new("verif-token", None);
    let payload = b"{}";
    let now = SystemTime::now();

    assert!(verifier
        .verify("evt-1", payload, "verif-token", now)
        .is_ok());
}

#[test]
fn feishu_verifier_rejects_invalid_token() {
    let verifier = FeishuWebhookVerifier::new("verif-token", None);
    let payload = b"{}";
    let now = SystemTime::now();

    let result = verifier.verify("evt-1", payload, "bad-token", now);
    assert_eq!(result, Err(WebhookError::SignatureMismatch));
}

#[test]
fn feishu_verifier_rejects_expired_timestamp() {
    let verifier = FeishuWebhookVerifier::new("verif-token", None).with_ttl(Duration::from_secs(5));
    let payload = b"{}";
    let old = SystemTime::now() - Duration::from_secs(10);

    let result = verifier.verify("evt-1", payload, "verif-token", old);
    assert_eq!(result, Err(WebhookError::TimestampExpired));
}

#[test]
fn feishu_verifier_rejects_replayed_event() {
    let verifier = FeishuWebhookVerifier::new("verif-token", None);
    let payload = b"{}";
    let now = SystemTime::now();

    verifier
        .verify("evt-replay", payload, "verif-token", now)
        .unwrap();
    let result = verifier.verify("evt-replay", payload, "verif-token", now);
    assert_eq!(result, Err(WebhookError::EventReplayed));
}

// ---------------------------------------------------------------------------
// Parse message content tests
// ---------------------------------------------------------------------------

#[test]
fn feishu_parse_text_message() {
    let parsed = parse_message_content("text", r#"{"text":"hello world"}"#).unwrap();
    assert_eq!(
        parsed,
        ParsedMessageContent::Text {
            text: "hello world".to_string(),
        }
    );
}

#[test]
fn feishu_parse_file_message_with_attachment() {
    let parsed = parse_message_content(
        "file",
        r#"{"file_key":"file-abc","file_name":"report.pdf","file_size":1024}"#,
    )
    .unwrap();
    assert_eq!(
        parsed,
        ParsedMessageContent::File {
            file_key: "file-abc".to_string(),
            file_name: "report.pdf".to_string(),
            file_size: 1024,
        }
    );
}

#[test]
fn feishu_parse_image_message() {
    let parsed = parse_message_content("image", r#"{"image_key":"img-xyz"}"#).unwrap();
    assert_eq!(
        parsed,
        ParsedMessageContent::Image {
            image_key: "img-xyz".to_string(),
        }
    );
}

#[test]
fn feishu_parse_unsupported_message_type() {
    let result = parse_message_content("sticker", r"{}");
    assert_eq!(
        result,
        Err(FeishuParseError::UnsupportedMessageType(
            "sticker".to_string()
        ))
    );
}

// ---------------------------------------------------------------------------
// Parse full event tests
// ---------------------------------------------------------------------------

#[test]
fn feishu_parse_p2p_chat() {
    let json = sample_event_json();
    let event = parse_feishu_event(json.as_bytes()).unwrap();
    assert_eq!(event.event.message.chat_type, "p2p");
    assert_eq!(event.event.message.parent_message_id, None);
}

#[test]
fn feishu_parse_group_chat() {
    let mut event = sample_event();
    event.event.message.chat_type = "group".to_string();
    let json = serde_json::to_string(&event).unwrap();
    let parsed = parse_feishu_event(json.as_bytes()).unwrap();
    assert_eq!(parsed.event.message.chat_type, "group");
}

#[test]
fn feishu_parse_thread_reply() {
    let mut event = sample_event();
    event.event.message.parent_message_id = Some("parent-msg-1".to_string());
    let json = serde_json::to_string(&event).unwrap();
    let parsed = parse_feishu_event(json.as_bytes()).unwrap();
    assert_eq!(
        parsed.event.message.parent_message_id,
        Some("parent-msg-1".to_string())
    );
}

// ---------------------------------------------------------------------------
// Event to ChannelEvent mapping
// ---------------------------------------------------------------------------

#[test]
fn feishu_event_to_channel_event_mapping() {
    let event = sample_event();
    let channel_event = feishu_event_to_channel_event(&event, resolved_identity()).unwrap();

    assert_eq!(channel_event.event_id, "evt-123");
    assert_eq!(channel_event.event_type, "channel.message.received");
    assert_eq!(channel_event.provider_tenant_id, "tenant-1");
    assert_eq!(channel_event.channel_binding_id, "chat-1");
    assert_eq!(channel_event.provider_user_id, "open-1");
    assert_eq!(channel_event.payload.text, "hello feishu");
    assert!(channel_event.payload.attachments.is_empty());
    assert_eq!(channel_event.seaki_workspace_id, "ws-1");
    assert_eq!(channel_event.seaki_actor_id, "actor-1");
    assert_eq!(channel_event.workspace_role, "member");
    assert_eq!(
        channel_event.channel_scope,
        "workspace:ws-1/channel:chat-1/user:open-1"
    );
}

#[test]
fn feishu_event_to_channel_event_with_file_attachment() {
    let mut event = sample_event();
    event.event.message.msg_type = "file".to_string();
    event.event.message.content =
        r#"{"file_key":"file-abc","file_name":"doc.pdf","file_size":2048}"#.to_string();

    let channel_event = feishu_event_to_channel_event(&event, resolved_identity()).unwrap();
    assert_eq!(channel_event.payload.text, "");
    assert_eq!(channel_event.payload.attachments.len(), 1);

    let att = &channel_event.payload.attachments[0];
    assert_eq!(att.provider, "feishu");
    assert_eq!(att.provider_file_key, "file-abc");
    assert_eq!(att.original_name, "doc.pdf");
    assert_eq!(att.declared_size, 2048);
    assert_eq!(att.provider_tenant_id, "tenant-1");
    assert_eq!(att.provider_chat_id, "chat-1");
    assert_eq!(att.provider_message_id, "msg-1");
    assert!(att.download_capability_required);
}

// ---------------------------------------------------------------------------
// Outbound payload tests
// ---------------------------------------------------------------------------

#[test]
fn feishu_build_reply_payload() {
    let payload = build_feishu_reply_payload("Hello back", None);
    let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(value["text"].as_str().unwrap(), "Hello back");
    assert!(value.get("root_id").is_none());
}

#[test]
fn feishu_build_thread_reply() {
    let payload = build_feishu_reply_payload("Thread reply", Some("parent-1"));
    let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
    assert_eq!(value["text"].as_str().unwrap(), "Thread reply");
    assert_eq!(value["root_id"].as_str().unwrap(), "parent-1");
    assert!(value["reply_in_thread"].as_bool().unwrap());
}

#[test]
fn feishu_reply_with_provenance() {
    let provenance = FeishuProvenance {
        transaction_id: "tx-42".to_string(),
        source_id: "src-42".to_string(),
        wiki_patch_hash: Some("abc123".to_string()),
        citation_ids: vec!["c-1".to_string(), "c-2".to_string()],
        audit_id: "audit-42".to_string(),
    };
    let text = format_reply_with_provenance("Here is the answer", &provenance);

    assert!(text.contains("Here is the answer"));
    assert!(text.contains("transaction: tx-42"));
    assert!(text.contains("source: src-42"));
    assert!(text.contains("wiki: abc123"));
    assert!(text.contains("citations: c-1, c-2"));
    assert!(text.contains("audit: audit-42"));
}

// ---------------------------------------------------------------------------
// Adapter build_outbound tests
// ---------------------------------------------------------------------------

#[test]
fn feishu_adapter_build_outbound_success() {
    let adapter = FeishuChannelAdapter::new();
    let grant = sample_grant("workspace:ws-1/channel:chat-1/user:open-1");
    let provenance = FeishuProvenance {
        transaction_id: "tx-99".to_string(),
        source_id: "src-99".to_string(),
        wiki_patch_hash: None,
        citation_ids: vec![],
        audit_id: "audit-99".to_string(),
    };

    let req = adapter
        .build_outbound(&grant, "Reply text", &provenance)
        .unwrap();
    assert_eq!(req.receive_id, "chat-1");
    assert_eq!(req.receive_id_type, "chat_id");
    assert_eq!(req.msg_type, "text");
    assert_eq!(req.uuid, "idem-1");
    assert!(!req.reply_in_thread);

    let content: serde_json::Value = serde_json::from_str(&req.content).unwrap();
    let text = content["text"].as_str().unwrap();
    assert!(text.contains("Reply text"));
    assert!(text.contains("transaction: tx-99"));
}

#[test]
fn feishu_adapter_build_outbound_scope_missing_channel() {
    let adapter = FeishuChannelAdapter::new();
    let grant = sample_grant("workspace:ws-1/user:open-1");
    let provenance = FeishuProvenance {
        transaction_id: "tx-99".to_string(),
        source_id: "src-99".to_string(),
        wiki_patch_hash: None,
        citation_ids: vec![],
        audit_id: "audit-99".to_string(),
    };

    let result = adapter.build_outbound(&grant, "Reply text", &provenance);
    assert!(
        matches!(
            result,
            Err(FeishuAdapterError::Parse(FeishuParseError::MissingField(
                "channel in scope"
            )))
        ),
        "expected MissingField error, got {:?}",
        result
    );
}
