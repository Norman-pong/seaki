//! M1 E2E: Fake Channel 入站 + Webhook + Outbox

use seaki_channel::fake_provider::{BindingEntry, ChannelMessagePayload, FakeChannelProvider};
use seaki_channel::grant::{ChannelResourceGrant, ChannelResourceGrantStore, GrantError};
use seaki_channel::outbox::{
    FakeProviderQueryAPI, Outbox, OutboxItem, OutboxStatus, ProviderQueryResult,
};
use seaki_channel::webhook::{hex_encode, hmac_sha256, WebhookError, WEBHOOK_SECRET};
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

fn sample_grant(id: &str) -> ChannelResourceGrant {
    ChannelResourceGrant {
        grant_id: id.to_string(),
        scope: "scope-1".to_string(),
        provider_tenant_id: "tenant-1".to_string(),
        provider_chat_id: "chat-1".to_string(),
        provider_message_id: "msg-1".to_string(),
        file_key: "file-1".to_string(),
        version: "v1".to_string(),
        seaki_actor_id: "actor-1".to_string(),
        operation: "read".to_string(),
        audience: "workspace".to_string(),
        idempotency_key: "idem-1".to_string(),
        uses_remaining: 2,
        issued_at: SystemTime::UNIX_EPOCH + Duration::from_secs(100),
        expires_at: SystemTime::UNIX_EPOCH + Duration::from_secs(1000),
    }
}

fn valid_signature(payload: &[u8]) -> String {
    hex_encode(&hmac_sha256(WEBHOOK_SECRET.as_bytes(), payload))
}

#[test]
fn m1_channel_bridge_webhook_to_outbox_happy_path() {
    // 1. FakeChannelProvider 配置 binding
    let provider = FakeChannelProvider::new();
    provider.upsert_binding(binding());
    let payload = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();
    let sig = valid_signature(payload);

    // 2. 提交合法 webhook payload，验证通过
    let event = provider
        .submit_event(
            payload,
            &sig,
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            ChannelMessagePayload {
                text: "hello".to_string(),
                attachments: Vec::new(),
            },
        )
        .expect("valid webhook should pass");
    assert_eq!(event.event_id, "evt-1");
    assert_eq!(event.provider_user_id, "user-1");

    // 3. 同一 event_id 再次提交，验证返回 EventReplayed
    let replay_result = provider.submit_event(
        payload,
        &sig,
        now,
        "evt-1",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        ChannelMessagePayload {
            text: "hello".to_string(),
            attachments: Vec::new(),
        },
    );
    assert_eq!(replay_result, Err(WebhookError::EventReplayed));

    // 4. 验证 guest role 请求 ChannelResourceGrant 被 policy 拒绝
    let grant_store = ChannelResourceGrantStore::new();
    let grant = sample_grant("g1");
    let grant_result = grant_store.issue("guest", grant);
    assert_eq!(grant_result, Err(GrantError::PolicyDeniedInsufficientRole));

    // 5. Outbox enqueue item，验证 idempotency key 不能重复
    let outbox = Outbox::new();
    let item = OutboxItem {
        id: "o1".to_string(),
        channel_event_id: "evt-1".to_string(),
        payload: String::from_utf8_lossy(payload).to_string(),
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
    };
    outbox.enqueue(item.clone()).expect("enqueue succeeds");

    // 先标记为 Sent，使 idempotency key 进入已发送集合
    outbox
        .transition("o1", &OutboxStatus::Pending, &OutboxStatus::Sending)
        .unwrap();
    outbox
        .transition("o1", &OutboxStatus::Sending, &OutboxStatus::Sent)
        .unwrap();

    let mut duplicate = item.clone();
    duplicate.id = "o2".to_string();
    let dup_result = outbox.enqueue(duplicate);
    assert_eq!(dup_result, Err("idempotency key already sent"));

    // 6. 模拟 unknown 状态，调用 FakeProviderQueryAPI 后 retry
    let mut unknown_item = item.clone();
    unknown_item.id = "o3".to_string();
    unknown_item.provider_idempotency_key = "idem-3".to_string();
    unknown_item.status = OutboxStatus::Unknown;
    outbox.enqueue(unknown_item).expect("enqueue unknown");

    struct NotFoundQueryAPI;
    impl FakeProviderQueryAPI for NotFoundQueryAPI {
        fn query(&self, _key: &str) -> ProviderQueryResult {
            ProviderQueryResult::NotFound
        }
    }

    let resolved = outbox
        .resolve_unknown("o3", &NotFoundQueryAPI)
        .expect("resolve unknown");
    assert_eq!(resolved, OutboxStatus::Retry);
    assert_eq!(outbox.item("o3").unwrap().status, OutboxStatus::Retry);

    // 7. 验证并发 lease 仅一人成功
    let outbox_arc = Arc::new(Outbox::new());
    outbox_arc
        .enqueue(OutboxItem {
            id: "o4".to_string(),
            channel_event_id: "evt-4".to_string(),
            payload: "{}".to_string(),
            provider_idempotency_key: "idem-4".to_string(),
            status: OutboxStatus::Pending,
            created_at: SystemTime::now(),
            lease_expires_at: None,
            lease_holder: None,
            transaction_id: "tx-4".to_string(),
            payload_hash: "hash".to_string(),
            scope: "scope".to_string(),
            audience: "audience".to_string(),
            provider_request_id: None,
            compensating_action: None,
            attempt_count: 0,
            next_attempt_at: None,
            last_error_code: None,
        })
        .expect("enqueue for lease race");

    let mut handles = Vec::new();
    for i in 0..8 {
        let o = Arc::clone(&outbox_arc);
        handles.push(thread::spawn(move || {
            o.lease(
                "o4",
                &format!("worker-{i}"),
                Duration::from_secs(30),
                SystemTime::now(),
            )
        }));
    }

    let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let wins = results.iter().filter(|&&b| b).count();
    assert_eq!(wins, 1, "only one concurrent lease should succeed");
}
