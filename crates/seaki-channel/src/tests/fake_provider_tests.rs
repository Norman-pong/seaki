use super::*;
use crate::webhook::WEBHOOK_SECRET;
use crate::webhook::{hex_encode, hmac_sha256};
use std::time::SystemTime;

fn sig(payload: &[u8]) -> String {
    hex_encode(&hmac_sha256(WEBHOOK_SECRET.as_bytes(), payload))
}

fn binding() -> BindingEntry {
    BindingEntry {
        provider_tenant_id: "tenant-1".to_string(),
        channel_binding_id: "bind-1".to_string(),
        provider_user_id: "user-1".to_string(),
        seaki_actor_id: "actor-1".to_string(),
        workspace_role: "member".to_string(),
    }
}

#[test]
fn provider_cannot_declare_seaki_actor_id() {
    let provider = FakeChannelProvider::new();
    provider.upsert_binding(binding());
    let payload = b"{\"text\":\"hi\"}";
    let now = SystemTime::now();

    // submit_event does NOT accept seaki_actor_id; it is resolved from binding.
    let event = provider
        .submit_event(
            payload,
            &sig(payload),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            ChannelMessagePayload {
                text: "hi".to_string(),
                attachments: Vec::new(),
            },
        )
        .expect("submit should succeed");

    // The returned event does NOT expose seaki_actor_id directly.
    assert_eq!(event.provider_tenant_id, "tenant-1");
    assert_eq!(event.provider_user_id, "user-1");
}

#[test]
fn unbound_identity_rejected() {
    let provider = FakeChannelProvider::new();
    // no binding inserted
    let payload = b"{\"text\":\"hi\"}";
    let now = SystemTime::now();

    let result = provider.submit_event(
        payload,
        &sig(payload),
        now,
        "evt-1",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        ChannelMessagePayload {
            text: "hi".to_string(),
            attachments: Vec::new(),
        },
    );
    assert_eq!(result, Err(WebhookError::SignatureMismatch));
}

#[test]
fn binding_crud_works() {
    let provider = FakeChannelProvider::new();
    let b = binding();
    provider.upsert_binding(b.clone());

    let resolved = provider.resolve_actor("tenant-1", "bind-1", "user-1");
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap().seaki_actor_id, "actor-1");

    let removed = provider.remove_binding("tenant-1", "bind-1", "user-1");
    assert!(removed.is_some());

    let resolved2 = provider.resolve_actor("tenant-1", "bind-1", "user-1");
    assert!(resolved2.is_none());
}

#[test]
fn webhook_verification_failure_propagated() {
    let provider = FakeChannelProvider::new();
    provider.upsert_binding(binding());
    let payload = b"{\"text\":\"hi\"}";
    let now = SystemTime::now();

    let result = provider.submit_event(
        payload,
        "bad-sig",
        now,
        "evt-1",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        ChannelMessagePayload {
            text: "hi".to_string(),
            attachments: Vec::new(),
        },
    );
    assert_eq!(result, Err(WebhookError::SignatureMismatch));
}
