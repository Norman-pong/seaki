use seaki_channel::ingress::{
    IdentityResolver, InMemoryIdentityResolver, IngressError, IngressNormalizer, IngressResult,
    ResolvedIdentity, UnmappedUserPolicy,
};
use seaki_channel::webhook::WebhookVerifier;
use seaki_channel::webhook::{hex_encode, hmac_sha256, FakeWebhookVerifier, WebhookError};
use seaki_channel::ChannelMessagePayload;
use std::time::{Duration, SystemTime};

const SECRET: &str = "seaki-fake-channel-webhook-secret";

fn valid_sig(payload: &[u8]) -> String {
    hex_encode(&hmac_sha256(SECRET.as_bytes(), payload))
}

fn resolver_with_binding() -> InMemoryIdentityResolver {
    let r = InMemoryIdentityResolver::new();
    r.upsert(
        "tenant-1",
        "bind-1",
        "user-1",
        ResolvedIdentity {
            seaki_workspace_id: "ws-1".to_string(),
            seaki_actor_id: "actor-1".to_string(),
            workspace_role: "member".to_string(),
        },
    );
    r
}

fn normalizer(
    policy: UnmappedUserPolicy,
) -> IngressNormalizer<FakeWebhookVerifier, InMemoryIdentityResolver> {
    let verifier = FakeWebhookVerifier::new(SECRET);
    let resolver = resolver_with_binding();
    IngressNormalizer::new(verifier, resolver, policy)
}

fn payload() -> ChannelMessagePayload {
    ChannelMessagePayload {
        text: "hello".to_string(),
        attachments: Vec::new(),
    }
}

// ---- normalize success ----

#[test]
fn normalize_success() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let event = n
        .normalize(
            raw,
            &valid_sig(raw),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            payload(),
        )
        .expect("normalize should succeed");

    assert_eq!(event.event_id, "evt-1");
    assert_eq!(event.event_type, "channel.message.v1");
    assert_eq!(event.provider_tenant_id, "tenant-1");
    assert_eq!(event.channel_binding_id, "bind-1");
    assert_eq!(event.provider_user_id, "user-1");
    assert_eq!(event.seaki_workspace_id, "ws-1");
    assert_eq!(event.seaki_actor_id, "actor-1");
    assert_eq!(event.workspace_role, "member");
    assert_eq!(
        event.channel_scope,
        "workspace:ws-1/channel:bind-1/user:user-1"
    );
    assert_eq!(event.payload.text, "hello");
}

// ---- webhook rejection ----

#[test]
fn normalize_rejects_forged_signature() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let err = n
        .normalize(
            raw,
            "bad-sig",
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            payload(),
        )
        .unwrap_err();

    assert_eq!(err, IngressError::Webhook(WebhookError::SignatureMismatch));

    let audits = n.audit_for_event("evt-1");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].result, IngressResult::RejectedSignature);
}

#[test]
fn normalize_rejects_expired_timestamp() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let old = SystemTime::now() - Duration::from_secs(400);

    let err = n
        .normalize(
            raw,
            &valid_sig(raw),
            old,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            payload(),
        )
        .unwrap_err();

    assert_eq!(err, IngressError::Webhook(WebhookError::TimestampExpired));

    let audits = n.audit_for_event("evt-1");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].result, IngressResult::RejectedExpired);
}

#[test]
fn normalize_rejects_replayed_event_id() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    n.normalize(
        raw,
        &valid_sig(raw),
        now,
        "evt-replay",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        payload(),
    )
    .unwrap();

    let err = n
        .normalize(
            raw,
            &valid_sig(raw),
            now,
            "evt-replay",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            payload(),
        )
        .unwrap_err();

    assert_eq!(err, IngressError::ReplayDetected);

    let audits = n.audit_for_event("evt-replay");
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].result, IngressResult::Accepted);
    assert_eq!(audits[1].result, IngressResult::RejectedReplay);
}

// ---- unmapped user policy ----

#[test]
fn normalize_unmapped_user_rejected() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let err = n
        .normalize(
            raw,
            &valid_sig(raw),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "unknown-user",
            payload(),
        )
        .unwrap_err();

    assert_eq!(err, IngressError::IdentityNotFound);

    let audits = n.audit_for_event("evt-1");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].result, IngressResult::RejectedUnmapped);
    assert!(audits[0].seaki_actor_id.is_none());
}

#[test]
fn normalize_unmapped_user_guest() {
    let n = normalizer(UnmappedUserPolicy::Guest);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let event = n
        .normalize(
            raw,
            &valid_sig(raw),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "unknown-user",
            payload(),
        )
        .expect("guest should be accepted");

    assert_eq!(event.seaki_actor_id, "guest:unknown-user");
    assert_eq!(event.workspace_role, "guest");
    assert_eq!(event.seaki_workspace_id, "default");
}

// ---- audit ----

#[test]
fn normalize_audit_accepted() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    n.normalize(
        raw,
        &valid_sig(raw),
        now,
        "evt-audit-ok",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        payload(),
    )
    .unwrap();

    let audits = n.audit_for_event("evt-audit-ok");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].result, IngressResult::Accepted);
    assert_eq!(audits[0].seaki_actor_id.as_deref(), Some("actor-1"));
    assert_eq!(audits[0].provider_tenant_id, "tenant-1");
    assert_eq!(audits[0].channel_binding_id, "bind-1");
    assert_eq!(audits[0].provider_user_id, "user-1");
}

#[test]
fn normalize_audit_rejected_signature() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let _ = n.normalize(
        raw,
        "bad-sig",
        now,
        "evt-audit-sig",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        payload(),
    );

    let audits = n.audit_for_event("evt-audit-sig");
    assert_eq!(audits.len(), 1);
    assert_eq!(audits[0].result, IngressResult::RejectedSignature);
    assert!(audits[0].seaki_actor_id.is_none());
}

// ---- identity resolver ----

#[test]
fn identity_resolver_upsert_and_resolve() {
    let r = InMemoryIdentityResolver::new();
    r.upsert(
        "t1",
        "b1",
        "u1",
        ResolvedIdentity {
            seaki_workspace_id: "ws-a".to_string(),
            seaki_actor_id: "actor-a".to_string(),
            workspace_role: "admin".to_string(),
        },
    );

    let id = r.resolve("t1", "b1", "u1").unwrap();
    assert_eq!(id.seaki_workspace_id, "ws-a");
    assert_eq!(id.seaki_actor_id, "actor-a");
    assert_eq!(id.workspace_role, "admin");
}

#[test]
fn identity_resolver_remove() {
    let r = InMemoryIdentityResolver::new();
    r.upsert(
        "t1",
        "b1",
        "u1",
        ResolvedIdentity {
            seaki_workspace_id: "ws-a".to_string(),
            seaki_actor_id: "actor-a".to_string(),
            workspace_role: "admin".to_string(),
        },
    );

    assert!(r.resolve("t1", "b1", "u1").is_some());
    let removed = r.remove("t1", "b1", "u1");
    assert!(removed.is_some());
    assert!(r.resolve("t1", "b1", "u1").is_none());
}

// ---- error display ----

#[test]
fn ingress_error_display() {
    assert_eq!(
        IngressError::Webhook(WebhookError::SignatureMismatch).to_string(),
        "webhook verification failed: SIGNATURE_MISMATCH"
    );
    assert_eq!(
        IngressError::IdentityNotFound.to_string(),
        "identity not found in binding table"
    );
    assert_eq!(
        IngressError::InvalidPayload("bad json".to_string()).to_string(),
        "invalid payload: bad json"
    );
    assert_eq!(
        IngressError::ReplayDetected.to_string(),
        "event replay detected"
    );
}

// ---- format checks ----

#[test]
fn guest_identity_format() {
    let n = normalizer(UnmappedUserPolicy::Guest);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let event = n
        .normalize(
            raw,
            &valid_sig(raw),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "uid-42",
            payload(),
        )
        .unwrap();

    assert_eq!(event.seaki_actor_id, "guest:uid-42");
}

#[test]
fn channel_scope_format() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let event = n
        .normalize(
            raw,
            &valid_sig(raw),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            payload(),
        )
        .unwrap();

    assert_eq!(
        event.channel_scope,
        "workspace:ws-1/channel:bind-1/user:user-1"
    );
}

#[test]
fn normalized_event_has_all_fields() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    let event = n
        .normalize(
            raw,
            &valid_sig(raw),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            payload(),
        )
        .unwrap();

    assert!(!event.event_id.is_empty());
    assert!(!event.event_type.is_empty());
    assert!(!event.provider_tenant_id.is_empty());
    assert!(!event.channel_binding_id.is_empty());
    assert!(!event.provider_user_id.is_empty());
    assert!(!event.seaki_workspace_id.is_empty());
    assert!(!event.seaki_actor_id.is_empty());
    assert!(!event.workspace_role.is_empty());
    assert!(!event.channel_scope.is_empty());
    // signature_verified_at and normalized_at are set to SystemTime::now()
    assert!(event.signature_verified_at > SystemTime::UNIX_EPOCH);
    assert!(event.normalized_at > SystemTime::UNIX_EPOCH);
}

#[test]
fn audit_log_accumulates() {
    let n = normalizer(UnmappedUserPolicy::Reject);
    let raw = b"{\"text\":\"hello\"}";
    let now = SystemTime::now();

    // 2 accepted + 1 rejected signature = 3 records
    n.normalize(
        raw,
        &valid_sig(raw),
        now,
        "evt-a",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        payload(),
    )
    .unwrap();
    n.normalize(
        raw,
        &valid_sig(raw),
        now,
        "evt-b",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        payload(),
    )
    .unwrap();
    let _ = n.normalize(
        raw,
        "bad-sig",
        now,
        "evt-c",
        "channel.message.v1",
        "tenant-1",
        "bind-1",
        "user-1",
        payload(),
    );

    assert_eq!(n.audit_log().len(), 3);
}

// ---- WebhookVerifier trait object compatibility ----

#[test]
fn trait_object_verify_delegates() {
    let verifier: Box<dyn WebhookVerifier> = Box::new(FakeWebhookVerifier::new(SECRET));
    let raw = b"{\"text\":\"hello\"}";
    let sig = valid_sig(raw);
    let now = SystemTime::now();

    assert!(verifier.verify("evt-1", raw, &sig, now).is_ok());
    assert_eq!(
        verifier.verify("evt-2", raw, "bad", now),
        Err(WebhookError::SignatureMismatch)
    );
}

#[test]
fn webhook_verifier_enforces_seen_event_id_bound() {
    let verifier = FakeWebhookVerifier::new(SECRET);
    let raw = b"{\"text\":\"hello\"}";
    let sig = valid_sig(raw);
    let now = SystemTime::now();

    // Insert more than MAX_SEEN_EVENT_IDS unique event ids.
    for i in 0..10_005 {
        let _ = verifier.verify(&format!("evt-{i}"), raw, &sig, now);
    }

    // The oldest events should have been evicted, so they can be re-verified.
    assert!(verifier.verify("evt-0", raw, &sig, now).is_ok());
    assert!(verifier.verify("evt-1", raw, &sig, now).is_ok());

    // Recent events should still be replay-rejected.
    assert_eq!(
        verifier.verify("evt-10004", raw, &sig, now),
        Err(WebhookError::EventReplayed)
    );
}
