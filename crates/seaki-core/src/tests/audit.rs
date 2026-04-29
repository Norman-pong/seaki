use super::*;

#[test]
fn invalid_schema_scope_and_duplicate_idempotency_do_not_write_events_or_audit() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    let mut invalid_schema = test_event("event-2", "idem-2", "workspace.note");
    invalid_schema.schema_version = CURRENT_EVENT_SCHEMA_VERSION + 1;
    assert!(matches!(
        ledger.append_inert_event(invalid_schema),
        Err(CoreError::InvalidSchemaVersion { .. })
    ));

    let mut invalid_payload_schema_hash = test_event("event-3", "idem-3", "workspace.note");
    invalid_payload_schema_hash.payload_schema_hash = "other.schema.v1".to_string();
    assert!(matches!(
        ledger.append_inert_event(invalid_payload_schema_hash),
        Err(CoreError::InvalidPayloadSchemaHash { .. })
    ));

    let mut invalid_scope = test_event("event-3", "idem-3", "workspace.note");
    invalid_scope.scope = "workspace:other".to_string();
    assert!(matches!(
        ledger.append_inert_event(invalid_scope),
        Err(CoreError::InvalidScope { .. })
    ));

    assert!(matches!(
        ledger.append_inert_event(test_event("event-4", "idem-1", "workspace.note")),
        Err(CoreError::DuplicateIdempotencyKey(_))
    ));

    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
}

#[test]
fn audit_hash_chain_appends_and_uses_sanitized_summary() {
    let mut ledger = initialized_ledger();
    ledger
        .append_inert_event(test_event("event-2", "idem-2", "workspace.token_observed"))
        .expect("event appends");

    let audit_entries = ledger.audit_entries().expect("audit entries load");
    assert_eq!(audit_entries.len(), 2);
    assert_eq!(audit_entries[0].previous_hash, GENESIS_AUDIT_HASH);
    assert_eq!(audit_entries[1].previous_hash, audit_entries[0].hash);
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));

    let replayed = ledger.replay_events_after(1).expect("events replay");
    assert_eq!(replayed.len(), 1);
    assert!(!replayed[0].payload_summary.contains("abc123"));
    assert!(!replayed[0].payload_summary.contains("super-secret"));
    assert!(!replayed[0]
        .payload_summary
        .to_lowercase()
        .contains("bearer"));
    assert!(replayed[0].payload_summary.contains("[REDACTED]"));
}

#[test]
fn audit_hash_chain_detects_event_tampering() {
    let mut ledger = initialized_ledger();
    ledger
        .append_inert_event(test_event("event-2", "idem-2", "workspace.note"))
        .expect("event appends");
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));

    ledger
        .conn
        .execute(
            "UPDATE events SET payload_summary = 'tampered after audit' WHERE seq = ?1",
            params![2_i64],
        )
        .expect("test tamper update succeeds");

    assert!(!ledger.verify_audit_chain().expect("audit chain loads"));
}
