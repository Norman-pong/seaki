use super::*;

#[test]
fn approved_decision_appends_wal_audit_and_record() {
    let mut ledger = initialized_ledger();
    let record = ApprovalDecisionRecord::new(
        "approval-1",
        "patch-1",
        ApprovalDecisionStatus::Approved,
        "user-1",
        None,
    );

    let envelope = ledger
        .append_approval_decision(ApprovalDecisionRequest::new(
            "event-2",
            "actor-1",
            "workspace-1",
            "idem-2",
            record.clone(),
        ))
        .expect("approval decision appends");

    assert_eq!(envelope.event_type, APPROVAL_DECISION_EVENT_TYPE);
    assert_eq!(
        envelope.payload_schema_hash,
        expected_payload_schema_hash(APPROVAL_DECISION_EVENT_TYPE)
    );
    assert!(envelope.payload_summary.contains("decision=approved"));
    assert!(envelope.payload_summary.contains("reason_present=false"));
    assert_eq!(
        ledger
            .approval_decision("approval-1")
            .expect("approval decision loads"),
        Some(record)
    );
    assert_eq!(
        ledger
            .workspace_revision("workspace-1")
            .expect("revision loads"),
        Some(2)
    );
    assert_eq!(ledger.audit_count().expect("audit count"), 2);
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));
}

#[test]
fn denied_decision_appends_wal_audit_with_sanitized_reason_summary() {
    let mut ledger = initialized_ledger();
    let reason = format!(
        "Bearer abc123 token=super-secret {}",
        "rejection explanation ".repeat(12)
    );

    let envelope = ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Denied,
            Some(reason),
        ))
        .expect("approval denial appends");

    assert_eq!(envelope.event_type, APPROVAL_DECISION_EVENT_TYPE);
    assert!(envelope.payload_summary.contains("decision=denied"));
    assert!(envelope.payload_summary.contains("reason_present=true"));
    assert!(!envelope.payload_summary.contains("abc123"));
    assert!(!envelope.payload_summary.contains("super-secret"));
    assert!(!envelope.payload_summary.to_lowercase().contains("bearer"));

    let stored = ledger
        .approval_decision("approval-1")
        .expect("approval decision loads")
        .expect("approval decision exists");
    let reason_summary = stored.reason_summary.expect("reason summary stored");
    assert!(stored.reason_present);
    assert!(reason_summary.chars().count() <= 96);
    assert!(!reason_summary.contains("abc123"));
    assert!(!reason_summary.contains("super-secret"));
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));
}

#[test]
fn approval_decision_validation_failures_do_not_write_events_or_audit() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    let mut invalid_schema = approval_decision_request(
        "event-2",
        "idem-2",
        "approval-1",
        "patch-1",
        ApprovalDecisionStatus::Approved,
        None,
    );
    invalid_schema.schema_version = CURRENT_EVENT_SCHEMA_VERSION + 1;
    assert!(matches!(
        ledger.append_approval_decision(invalid_schema),
        Err(CoreError::InvalidSchemaVersion { .. })
    ));

    let mut invalid_scope = approval_decision_request(
        "event-3",
        "idem-3",
        "approval-2",
        "patch-1",
        ApprovalDecisionStatus::Approved,
        None,
    );
    invalid_scope.scope = "workspace:other".to_string();
    assert!(matches!(
        ledger.append_approval_decision(invalid_scope),
        Err(CoreError::InvalidScope { .. })
    ));

    assert!(matches!(
        ledger.append_approval_decision(approval_decision_request(
            "event-4",
            "idem-1",
            "approval-3",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        )),
        Err(CoreError::DuplicateIdempotencyKey(_))
    ));

    assert!(matches!(
        ledger.append_approval_decision(approval_decision_request(
            "event-1",
            "idem-4",
            "approval-4",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        )),
        Err(CoreError::DuplicateEventId(_))
    ));

    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
    assert!(ledger
        .approval_decision("approval-1")
        .expect("approval decision loads")
        .is_none());
}

#[test]
fn approval_decision_workspace_missing_does_not_write() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    assert!(matches!(
        ledger.append_approval_decision(ApprovalDecisionRequest::new(
            "event-2",
            "actor-1",
            "workspace-missing",
            "idem-2",
            ApprovalDecisionRecord::new(
                "approval-1",
                "patch-1",
                ApprovalDecisionStatus::Denied,
                "user-1",
                Some("not enough citation evidence".to_string()),
            ),
        )),
        Err(CoreError::WorkspaceMissing(_))
    ));

    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
    assert!(ledger
        .approval_decision("approval-1")
        .expect("approval decision loads")
        .is_none());
}
