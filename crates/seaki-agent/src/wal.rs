use seaki_core::{CoreLedger, InertEvent, CURRENT_EVENT_SCHEMA_VERSION};

/// Errors that can occur when writing to the WAL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalError {
    LedgerError(String),
    SerializeError(String),
}

impl std::fmt::Display for WalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalError::LedgerError(msg) => write!(f, "ledger error: {msg}"),
            WalError::SerializeError(msg) => write!(f, "serialize error: {msg}"),
        }
    }
}

impl std::error::Error for WalError {}

/// Writes compaction summary to WAL.
///
/// # Arguments
/// * `ledger` — The core ledger to append the event to.
/// * `workspace_id` — The workspace this compaction belongs to.
/// * `session_id` — The session being compacted.
/// * `summary` — The compaction summary.
pub fn write_compaction_to_wal(
    ledger: &mut CoreLedger,
    workspace_id: &str,
    session_id: &str,
    summary: &crate::session::CompactionSummary,
) -> Result<(), WalError> {
    let payload_json =
        serde_json::to_string(summary).map_err(|e| WalError::SerializeError(e.to_string()))?;

    // Use a counter suffix to avoid idempotency_key collision within the same millisecond.
    static COMPACT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let seq = COMPACT_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let event = InertEvent {
        event_id: format!(
            "session-compact-{session_id}-{}-{seq}",
            summary.compacted_at_ms
        ),
        schema_version: CURRENT_EVENT_SCHEMA_VERSION,
        payload_schema_hash: "session.compacted.v1".to_string(),
        actor_id: "agent".to_string(),
        scope: seaki_core::workspace_scope(workspace_id),
        workspace_id: workspace_id.to_string(),
        idempotency_key: format!(
            "session-compact-{session_id}-{}-{seq}",
            summary.compacted_at_ms
        ),
        event_type: "session.compacted".to_string(),
        payload_summary: payload_json,
    };

    ledger
        .append_inert_event(event)
        .map_err(|e| WalError::LedgerError(e.to_string()))?;

    Ok(())
}
