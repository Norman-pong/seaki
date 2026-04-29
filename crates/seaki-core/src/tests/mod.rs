use super::*;
use seaki_index::{
    CandidateKind, IndexCandidateId, IndexGeneration, IndexScope, IndexedCitationRef,
    IndexedDocument, SourceRange, SourceRangeUnit, SourceStatus, Visibility,
};
use seaki_memory::{
    note::{memory_scope, NoteStore},
    redaction::RedactedSessionManifest,
    session_search::{session_scope, SessionCleanupAction, SessionSearchIndex},
};
use tempfile::NamedTempFile;

mod approval;
mod audit;
mod citation;
mod core;
mod memory;
mod pipe;
mod search;
mod session;
mod wiki_patch;
mod workspace;

fn initialized_ledger() -> CoreLedger {
    let mut ledger = CoreLedger::open_in_memory().expect("ledger opens");
    ledger
        .workspace_init(workspace_init_request("event-1", "idem-1"))
        .expect("workspace init succeeds");
    ledger
}

fn seed_search_index(ledger: &mut CoreLedger) {
    let scope = IndexScope::new("workspace-1", "account-1");
    ledger
        .replace_search_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [
                indexed_document(
                    "doc-visible",
                    &scope,
                    "source-1",
                    "needle",
                    "allowed cited body",
                    Visibility::Visible,
                    SourceStatus::Active,
                ),
                indexed_document(
                    "doc-restricted",
                    &scope,
                    "source-1",
                    "needle",
                    "restricted body",
                    Visibility::Restricted,
                    SourceStatus::Active,
                ),
            ],
        )
        .expect("search scope seeds");
}

fn indexed_document(
    id: &str,
    scope: &IndexScope,
    source_id: &str,
    title: &str,
    body: &str,
    visibility: Visibility,
    source_status: SourceStatus,
) -> IndexedDocument {
    IndexedDocument {
        candidate_id: IndexCandidateId::new(id),
        workspace_id: scope.workspace_id.clone(),
        account_id: scope.account_id.clone(),
        source_id: source_id.to_string(),
        citation_ref: Some(IndexedCitationRef {
            citation_id: format!("citation-{id}"),
            source_id: source_id.to_string(),
            range: SourceRange {
                unit: SourceRangeUnit::Line,
                start: 1,
                end: 1,
                label: Some(format!("{source_id}:1")),
            },
            wiki_page_id: format!("page-{id}"),
            claim_id: format!("claim-{id}"),
            degraded_reason: None,
        }),
        kind: CandidateKind::Claim,
        title: title.to_string(),
        body: body.to_string(),
        visibility,
        source_status,
        source_revision: 1,
        wiki_revision: 1,
    }
}

fn uncited_document(id: &str, scope: &IndexScope, title: &str, body: &str) -> IndexedDocument {
    let mut document = indexed_document(
        id,
        scope,
        "source-1",
        title,
        body,
        Visibility::Visible,
        SourceStatus::Active,
    );
    document.citation_ref = None;
    document
}

fn workspace_init_request(event_id: &str, idempotency_key: &str) -> WorkspaceInitRequest {
    WorkspaceInitRequest::new(
        event_id,
        "actor-1",
        "workspace-1",
        idempotency_key,
        "create workspace",
    )
}

fn wiki_patch_commit_request(
    event_id: &str,
    idempotency_key: &str,
    committed_revision: u64,
    transaction_id: &str,
    patch_id: &str,
    approval_id: &str,
    rollback_marker_id: &str,
) -> WikiPatchCommitRequest {
    WikiPatchCommitRequest::new(
        event_id,
        "actor-1",
        "workspace-1",
        idempotency_key,
        WikiPatchCommitRecord::new(
            transaction_id,
            patch_id,
            approval_id,
            committed_revision,
            rollback_marker_id,
        ),
    )
}

fn approval_decision_request(
    event_id: &str,
    idempotency_key: &str,
    approval_id: &str,
    patch_id: &str,
    decision: ApprovalDecisionStatus,
    reason: Option<String>,
) -> ApprovalDecisionRequest {
    ApprovalDecisionRequest::new(
        event_id,
        "actor-1",
        "workspace-1",
        idempotency_key,
        ApprovalDecisionRecord::new(approval_id, patch_id, decision, "user-1", reason),
    )
}

fn test_event(event_id: &str, idempotency_key: &str, event_type: &str) -> InertEvent {
    InertEvent {
        event_id: event_id.to_string(),
        schema_version: CURRENT_EVENT_SCHEMA_VERSION,
        actor_id: "actor-1".to_string(),
        scope: workspace_scope("workspace-1"),
        workspace_id: "workspace-1".to_string(),
        idempotency_key: idempotency_key.to_string(),
        event_type: event_type.to_string(),
        payload_schema_hash: expected_payload_schema_hash(event_type),
        payload_summary: "Bearer abc123 token=super-secret".to_string(),
    }
}

fn memory_propose_request(
    event_id: &str,
    idempotency_key: &str,
    note_id: &str,
) -> MemoryProposeRequest {
    MemoryProposeRequest::new(
        event_id,
        "actor-1",
        "workspace-1",
        idempotency_key,
        note_id,
        "test-title",
        "test content",
    )
}

fn memory_commit_request(
    event_id: &str,
    idempotency_key: &str,
    note_id: &str,
    approval_id: &str,
    committed_revision: u64,
) -> MemoryCommitRequest {
    MemoryCommitRequest::new(
        event_id,
        "actor-1",
        "workspace-1",
        idempotency_key,
        note_id,
        approval_id,
        committed_revision,
    )
}

// ---- M1 E2E: Pipeline dry-run + Proposal Artifact ----
