use super::*;

#[test]
fn wiki_patch_commit_appends_wal_audit_and_marks_index_stale() {
    let mut ledger = initialized_ledger();
    ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        ))
        .expect("approval decision appends");

    let envelope = ledger
        .append_wiki_patch_commit(wiki_patch_commit_request(
            "event-3",
            "idem-3",
            3,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        ))
        .expect("wiki patch commit appends");

    assert_eq!(envelope.event_type, WIKI_PATCH_COMMIT_EVENT_TYPE);
    assert_eq!(
        envelope.payload_schema_hash,
        expected_payload_schema_hash(WIKI_PATCH_COMMIT_EVENT_TYPE)
    );
    assert_eq!(
        ledger
            .workspace_revision("workspace-1")
            .expect("revision loads"),
        Some(3)
    );
    assert_eq!(
        ledger.index_status("workspace-1").expect("index status"),
        Some(INDEX_STATUS_STALE.to_string())
    );
    assert_eq!(ledger.audit_count().expect("audit count"), 3);
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));
}

#[test]
fn wiki_patch_commit_rejects_revision_mismatch_before_wal_write() {
    let mut ledger = initialized_ledger();
    ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        ))
        .expect("approval decision appends");
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    assert!(matches!(
        ledger.append_wiki_patch_commit(wiki_patch_commit_request(
            "event-3",
            "idem-3",
            99,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        )),
        Err(CoreError::WorkspaceRevisionMismatch {
            expected: 3,
            found: 99
        })
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
    assert_eq!(
        ledger
            .workspace_revision("workspace-1")
            .expect("revision loads"),
        Some(2)
    );
}

#[test]
fn wiki_patch_commit_rejects_missing_or_denied_approval_decision_before_wal_write() {
    let mut missing_ledger = initialized_ledger();
    let missing_events = missing_ledger.event_count().expect("event count");
    let missing_audit = missing_ledger.audit_count().expect("audit count");

    assert!(matches!(
        missing_ledger.append_wiki_patch_commit(wiki_patch_commit_request(
            "event-2",
            "idem-2",
            2,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        )),
        Err(CoreError::ApprovalDecisionRequired { .. })
    ));
    assert_eq!(
        missing_ledger.event_count().expect("event count"),
        missing_events
    );
    assert_eq!(
        missing_ledger.audit_count().expect("audit count"),
        missing_audit
    );

    let mut denied_ledger = initialized_ledger();
    denied_ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Denied,
            Some("citation does not support claim".to_string()),
        ))
        .expect("denied approval records");
    let denied_events = denied_ledger.event_count().expect("event count");
    let denied_audit = denied_ledger.audit_count().expect("audit count");

    assert!(matches!(
        denied_ledger.append_wiki_patch_commit(wiki_patch_commit_request(
            "event-3",
            "idem-3",
            3,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        )),
        Err(CoreError::ApprovalDecisionNotApproved { .. })
    ));
    assert_eq!(
        denied_ledger.event_count().expect("event count"),
        denied_events
    );
    assert_eq!(
        denied_ledger.audit_count().expect("audit count"),
        denied_audit
    );
}
