use super::*;

#[test]
fn workspace_init_returns_revision_audit_head_and_index_status() {
    let mut ledger = CoreLedger::open_in_memory().expect("ledger opens");
    let result = ledger
        .workspace_init(workspace_init_request("event-1", "idem-1"))
        .expect("workspace init succeeds");

    assert_eq!(result.workspace_id, "workspace-1");
    assert_eq!(result.workspace_revision, 1);
    assert_ne!(result.audit_head, GENESIS_AUDIT_HASH);
    assert_eq!(result.index_status, INDEX_STATUS_STALE);
    assert_eq!(
        ledger.audit_head("workspace-1").expect("audit head loads"),
        Some(result.audit_head)
    );
}

#[test]
fn duplicate_workspace_init_is_rejected_before_write() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    assert!(matches!(
        ledger.workspace_init(workspace_init_request("event-2", "idem-2")),
        Err(CoreError::WorkspaceAlreadyExists(_))
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
}

#[test]
fn empty_idempotency_key_is_rejected_before_write() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let mut event = test_event("event-2", "", "workspace.note");
    event.idempotency_key = "  ".to_string();

    assert!(matches!(
        ledger.append_inert_event(event),
        Err(CoreError::EmptyIdempotencyKey)
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
}
