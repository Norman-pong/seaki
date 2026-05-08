use seaki_channel::outbox::{
    FakeProviderQueryAPI, Outbox, OutboxItem, OutboxStatus, ProviderQueryResult,
};
use seaki_channel::webhook::{hex_encode, hmac_sha256, WEBHOOK_SECRET};
use seaki_channel::{BindingEntry, ChannelMessagePayload, FakeChannelProvider, WebhookError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

fn binding() -> BindingEntry {
    BindingEntry {
        provider_tenant_id: "tenant-1".to_string(),
        channel_binding_id: "bind-1".to_string(),
        provider_user_id: "user-1".to_string(),
        seaki_workspace_id: "ws-1".to_string(),
        seaki_actor_id: "actor-1".to_string(),
        workspace_role: "member".to_string(),
    }
}

// ---- Integration: fake provider + webhook ----

#[test]
fn provider_webhook_signature_timestamp_replay_errors() {
    let provider = FakeChannelProvider::new();
    provider.upsert_binding(binding());
    let payload = b"{\"text\":\"hi\"}";
    let now = SystemTime::now();
    let sig = hex_encode(&hmac_sha256(WEBHOOK_SECRET.as_bytes(), payload));

    // signature mismatch
    let r = provider.submit_event(
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
    assert_eq!(r, Err(WebhookError::SignatureMismatch));

    // timestamp expired
    let old = now - Duration::from_secs(400);
    let r = provider.submit_event(
        payload,
        &sig,
        old,
        "evt-2",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        ChannelMessagePayload {
            text: "hi".to_string(),
            attachments: Vec::new(),
        },
    );
    assert_eq!(r, Err(WebhookError::TimestampExpired));

    // replay
    let r = provider.submit_event(
        payload,
        &sig,
        now,
        "evt-3",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        ChannelMessagePayload {
            text: "hi".to_string(),
            attachments: Vec::new(),
        },
    );
    assert!(r.is_ok());
    let r = provider.submit_event(
        payload,
        &sig,
        now,
        "evt-3",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        ChannelMessagePayload {
            text: "hi".to_string(),
            attachments: Vec::new(),
        },
    );
    assert_eq!(r, Err(WebhookError::EventReplayed));
}

// ---- Integration: grant + outbox ----

#[test]
fn outbox_idempotency_and_unknown_flow() {
    let outbox = Outbox::new();
    let mut item = OutboxItem {
        id: "o1".to_string(),
        channel_event_id: "evt-1".to_string(),
        payload: "{}".to_string(),
        provider_idempotency_key: "idem-1".to_string(),
        status: OutboxStatus::Unknown,
        created_at: SystemTime::now(),
        lease_expires_at: None,
        lease_holder: None,
        transaction_id: "tx-1".to_string(),
        payload_hash: "hash".to_string(),
        scope: "scope".to_string(),
        audience: "audience".to_string(),
        provider_request_id: None,
        compensating_action: None,
        attempt_count: 0,
        next_attempt_at: None,
        last_error_code: None,
    };
    outbox.enqueue(item.clone()).unwrap();

    struct Q;
    impl FakeProviderQueryAPI for Q {
        fn query(&self, _key: &str) -> ProviderQueryResult {
            ProviderQueryResult::Sent
        }
    }

    let status = outbox.resolve_unknown("o1", &Q).unwrap();
    assert_eq!(status, OutboxStatus::Sent);

    // Same idempotency key cannot be enqueued again.
    item.id = "o2".to_string();
    let r = outbox.enqueue(item);
    assert_eq!(r, Err("idempotency key already sent"));
}

#[test]
fn concurrent_lease_race() {
    let outbox = Arc::new(Outbox::new());
    outbox
        .enqueue(OutboxItem {
            id: "o1".to_string(),
            channel_event_id: "evt-1".to_string(),
            payload: "{}".to_string(),
            provider_idempotency_key: "idem-1".to_string(),
            status: OutboxStatus::Pending,
            created_at: SystemTime::now(),
            lease_expires_at: None,
            lease_holder: None,
            transaction_id: "tx-1".to_string(),
            payload_hash: "hash".to_string(),
            scope: "scope".to_string(),
            audience: "audience".to_string(),
            provider_request_id: None,
            compensating_action: None,
            attempt_count: 0,
            next_attempt_at: None,
            last_error_code: None,
        })
        .unwrap();

    let mut handles = Vec::new();
    for i in 0..8 {
        let o = Arc::clone(&outbox);
        handles.push(thread::spawn(move || {
            o.lease(
                "o1",
                &format!("worker-{i}"),
                Duration::from_secs(30),
                SystemTime::now(),
            )
        }));
    }

    let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|&&b| b).count(), 1);
}
