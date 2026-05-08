use crate::session::CompactionSummary;
use crate::wal::{write_compaction_to_wal, WalError};
use seaki_core::{CoreLedger, WorkspaceInitRequest};

fn make_ledger() -> CoreLedger {
    CoreLedger::open_in_memory().expect("in-memory ledger should open")
}

fn init_workspace(ledger: &mut CoreLedger, workspace_id: &str) {
    let req = WorkspaceInitRequest::new(
        format!("evt-{workspace_id}"),
        "actor1",
        workspace_id,
        format!("idmp-{workspace_id}"),
        "init",
    );
    ledger
        .workspace_init(req)
        .expect("workspace init should succeed");
}

#[test]
fn wal_write_compaction_success() {
    let mut ledger = make_ledger();
    init_workspace(&mut ledger, "ws1");

    let summary = CompactionSummary {
        original_message_count: 10,
        removed_message_count: 5,
        retained_claim_count: 2,
        summary_text: "test summary".to_string(),
        compacted_at_ms: 1_700_000_000_000,
    };

    let result = write_compaction_to_wal(&mut ledger, "ws1", "session-1", &summary);
    assert!(
        result.is_ok(),
        "expected WAL write to succeed, got {:?}",
        result
    );
}

#[test]
fn wal_write_compaction_error() {
    let mut ledger = make_ledger();
    // No workspace initialized, so append_inert_event should fail.

    let summary = CompactionSummary {
        original_message_count: 10,
        removed_message_count: 5,
        retained_claim_count: 2,
        summary_text: "test summary".to_string(),
        compacted_at_ms: 1_700_000_000_000,
    };

    let result = write_compaction_to_wal(&mut ledger, "ws-no-init", "session-2", &summary);
    assert!(
        matches!(result, Err(WalError::LedgerError(_))),
        "expected LedgerError because workspace is missing, got {:?}",
        result
    );
}

#[test]
fn wal_error_display_and_source() {
    let ledger_err = WalError::LedgerError("db locked".to_string());
    assert_eq!(format!("{ledger_err}"), "ledger error: db locked");

    let serialize_err = WalError::SerializeError("bad json".to_string());
    assert_eq!(format!("{serialize_err}"), "serialize error: bad json");

    // WalError does not chain a source error.
    let err: &dyn std::error::Error = &ledger_err;
    assert!(err.source().is_none());
}
