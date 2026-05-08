use super::*;
use std::time::SystemTime;

struct AlwaysOkVerifier;

impl WebhookVerifier for AlwaysOkVerifier {
    fn verify(
        &self,
        _event_id: &str,
        _raw_payload: &[u8],
        _signature: &str,
        _timestamp: SystemTime,
    ) -> Result<(), WebhookError> {
        Ok(())
    }
}

struct AlwaysResolve;

impl IdentityResolver for AlwaysResolve {
    fn resolve(
        &self,
        _provider_tenant_id: &str,
        _channel_binding_id: &str,
        _provider_user_id: &str,
    ) -> Option<ResolvedIdentity> {
        Some(ResolvedIdentity {
            seaki_workspace_id: "ws-1".to_string(),
            seaki_actor_id: "actor-1".to_string(),
            workspace_role: "member".to_string(),
        })
    }
}

fn payload() -> ChannelMessagePayload {
    ChannelMessagePayload {
        text: "hi".to_string(),
        attachments: Vec::new(),
    }
}

#[test]
fn audit_log_rotates_at_max_size() {
    let normalizer =
        IngressNormalizer::new(AlwaysOkVerifier, AlwaysResolve, UnmappedUserPolicy::Reject);

    // Fill the audit log to capacity.
    for i in 0..10_005 {
        let _ = normalizer.normalize(
            b"{}",
            "sig",
            SystemTime::now(),
            &format!("evt-{i}"),
            "msg",
            "tenant",
            "bind",
            "user",
            payload(),
        );
    }

    let log = normalizer.audit_log();
    assert_eq!(
        log.len(),
        10_000,
        "audit log should be bounded by MAX_AUDIT_LOG_SIZE"
    );

    // The oldest entries (0..4) should have been evicted.
    assert!(
        !log.iter().any(|r| r.event_id == "evt-0"),
        "oldest entry should have been evicted"
    );
    assert!(
        !log.iter().any(|r| r.event_id == "evt-4"),
        "oldest entry should have been evicted"
    );
    assert!(
        log.iter().any(|r| r.event_id == "evt-5"),
        "entry 5 should still be present"
    );
    assert!(
        log.iter().any(|r| r.event_id == "evt-10004"),
        "newest entry should be present"
    );
}

#[test]
fn audit_log_records_accepted_and_rejected() {
    let normalizer =
        IngressNormalizer::new(AlwaysOkVerifier, AlwaysResolve, UnmappedUserPolicy::Reject);

    // Accepted
    let _ = normalizer.normalize(
        b"{}",
        "sig",
        SystemTime::now(),
        "evt-accept",
        "msg",
        "tenant",
        "bind",
        "user",
        payload(),
    );

    // Rejected by verifier (need a failing verifier)
    struct AlwaysFailVerifier;
    impl WebhookVerifier for AlwaysFailVerifier {
        fn verify(
            &self,
            _event_id: &str,
            _raw_payload: &[u8],
            _signature: &str,
            _timestamp: SystemTime,
        ) -> Result<(), WebhookError> {
            Err(WebhookError::SignatureMismatch)
        }
    }

    let failing_normalizer = IngressNormalizer::new(
        AlwaysFailVerifier,
        AlwaysResolve,
        UnmappedUserPolicy::Reject,
    );
    let _ = failing_normalizer.normalize(
        b"{}",
        "sig",
        SystemTime::now(),
        "evt-reject",
        "msg",
        "tenant",
        "bind",
        "user",
        payload(),
    );

    let log = normalizer.audit_log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].result, IngressResult::Accepted);
    assert_eq!(log[0].event_id, "evt-accept");

    let reject_log = failing_normalizer.audit_log();
    assert_eq!(reject_log.len(), 1);
    assert_eq!(reject_log[0].result, IngressResult::RejectedSignature);
    assert_eq!(reject_log[0].event_id, "evt-reject");
}
