use super::*;
use std::time::{Duration, SystemTime};

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

#[test]
fn guest_is_denied_resource_grant() {
    let store = ChannelResourceGrantStore::new();
    let grant = sample_grant("g1");
    let result = store.issue("guest", grant);
    assert_eq!(result, Err(GrantError::PolicyDeniedInsufficientRole));
}

#[test]
fn member_can_issue_and_consume() {
    let store = ChannelResourceGrantStore::new();
    let grant = sample_grant("g1");
    store.issue("member", grant.clone()).unwrap();

    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(500);
    store
        .consume(
            "g1",
            "scope-1",
            "file-1",
            "v1",
            "actor-1",
            "read",
            "workspace",
            now,
        )
        .unwrap();
    assert_eq!(store.get("g1").unwrap().uses_remaining, 1);

    store
        .consume(
            "g1",
            "scope-1",
            "file-1",
            "v1",
            "actor-1",
            "read",
            "workspace",
            now,
        )
        .unwrap();
    assert_eq!(store.get("g1").unwrap().uses_remaining, 0);

    let result = store.consume(
        "g1",
        "scope-1",
        "file-1",
        "v1",
        "actor-1",
        "read",
        "workspace",
        now,
    );
    assert_eq!(result, Err(GrantError::UsesExhausted));
}

#[test]
fn expired_grant_cannot_be_consumed() {
    let store = ChannelResourceGrantStore::new();
    let grant = sample_grant("g1");
    store.issue("member", grant).unwrap();

    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(2000);
    let result = store.consume(
        "g1",
        "scope-1",
        "file-1",
        "v1",
        "actor-1",
        "read",
        "workspace",
        now,
    );
    assert_eq!(result, Err(GrantError::GrantExpired));
}

#[test]
fn mismatch_fields_rejected() {
    let store = ChannelResourceGrantStore::new();
    let grant = sample_grant("g1");
    store.issue("member", grant).unwrap();

    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(500);
    assert_eq!(
        store.consume(
            "g1",
            "bad-scope",
            "file-1",
            "v1",
            "actor-1",
            "read",
            "workspace",
            now
        ),
        Err(GrantError::ScopeMismatch)
    );
    assert_eq!(
        store.consume(
            "g1",
            "scope-1",
            "bad-file",
            "v1",
            "actor-1",
            "read",
            "workspace",
            now
        ),
        Err(GrantError::FileKeyMismatch)
    );
    assert_eq!(
        store.consume(
            "g1",
            "scope-1",
            "file-1",
            "bad-version",
            "actor-1",
            "read",
            "workspace",
            now
        ),
        Err(GrantError::VersionMismatch)
    );
    assert_eq!(
        store.consume(
            "g1",
            "scope-1",
            "file-1",
            "v1",
            "bad-actor",
            "read",
            "workspace",
            now
        ),
        Err(GrantError::ActorMismatch)
    );
    assert_eq!(
        store.consume(
            "g1",
            "scope-1",
            "file-1",
            "v1",
            "actor-1",
            "bad-op",
            "workspace",
            now
        ),
        Err(GrantError::OperationMismatch)
    );
    assert_eq!(
        store.consume(
            "g1",
            "scope-1",
            "file-1",
            "v1",
            "actor-1",
            "read",
            "bad-audience",
            now
        ),
        Err(GrantError::AudienceMismatch)
    );
}

#[test]
fn fake_broker_produces_mock_metadata() {
    let broker = FakeBroker::new("/tmp/quarantine");
    let attachment = ChannelAttachmentRef {
        attachment_id: "att-1".to_string(),
        provider: "slack".to_string(),
        provider_tenant_id: "tenant-1".to_string(),
        provider_chat_id: "chat-1".to_string(),
        provider_message_id: "msg-1".to_string(),
        provider_thread_id: "thread-1".to_string(),
        provider_file_key: "key-1".to_string(),
        provider_file_version: "v2".to_string(),
        original_name: "photo.png".to_string(),
        declared_mime: "image/png".to_string(),
        declared_size: 1024,
        content_hash: None,
        download_capability_required: false,
    };

    let q = broker.download(&attachment);
    assert_eq!(q.file_key, "key-1");
    assert_eq!(q.version, "v2");
    assert_eq!(q.observed_mime, "image/png");
    assert_eq!(q.malware_scan_status, MalwareScanStatus::Clean);
    assert!(q.content_hash.contains("key-1"));
    assert!(q.quarantine_path.contains("/tmp/quarantine"));
    assert_eq!(q.observed_size, 0);
}
