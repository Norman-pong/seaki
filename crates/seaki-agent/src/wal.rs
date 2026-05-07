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
/// # Notes
/// This uses `session_id` as a placeholder for `workspace_id` because the
/// current function signature does not carry workspace context. When
/// `seaki-core` extends `InertEvent` with a dedicated session-compaction
/// variant this should be revisited.
pub fn write_compaction_to_wal(
    ledger: &mut CoreLedger,
    session_id: &str,
    summary: &crate::session::CompactionSummary,
) -> Result<(), WalError> {
    let payload_json =
        serde_json::to_string(summary).map_err(|e| WalError::SerializeError(e.to_string()))?;

    let event = InertEvent {
        event_id: format!("session-compact-{session_id}-{}", summary.compacted_at_ms),
        schema_version: CURRENT_EVENT_SCHEMA_VERSION,
        payload_schema_hash: "session.compacted.v1".to_string(),
        actor_id: "agent".to_string(),
        scope: seaki_core::workspace_scope(session_id),
        workspace_id: session_id.to_string(),
        idempotency_key: format!("session-compact-{session_id}-{}", summary.compacted_at_ms),
        event_type: "session.compacted".to_string(),
        payload_summary: payload_json,
    };

    ledger
        .append_inert_event(event)
        .map_err(|e| WalError::LedgerError(e.to_string()))?;

    Ok(())
}
