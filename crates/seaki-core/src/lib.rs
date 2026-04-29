use rusqlite::{params, Connection, OptionalExtension, Transaction};
use seaki_index::{
    Bm25CandidateIndex, CandidateKind, IndexGeneration, IndexStatus, IndexedCitationRef,
    IndexedDocument, SourceRangeUnit,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

pub const CORE_AUTHORITY: &str = "policy-approved-core";
pub const CURRENT_EVENT_SCHEMA_VERSION: u32 = 1;
pub const GENESIS_AUDIT_HASH: &str = "GENESIS";
pub const INDEX_STATUS_ERROR: &str = "error";
pub const INDEX_STATUS_FRESH: &str = "fresh";
pub const INDEX_STATUS_STALE: &str = "stale";
pub const APPROVAL_DECISION_APPROVED: &str = "approved";
pub const APPROVAL_DECISION_DENIED: &str = "denied";
pub const APPROVAL_DECISION_EVENT_TYPE: &str = "approval.decided";
pub const WIKI_PATCH_COMMIT_EVENT_TYPE: &str = "wiki.patch.commit";
pub const MEMORY_PROPOSE_EVENT_TYPE: &str = "memory.proposed";
pub const MEMORY_COMMIT_EVENT_TYPE: &str = "memory.committed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreRecordKind {
    Task,
    Transaction,
    AuditEvent,
    ApprovalDecision,
}

#[must_use]
pub fn owns_record_kind(kind: CoreRecordKind) -> bool {
    matches!(
        kind,
        CoreRecordKind::Task
            | CoreRecordKind::Transaction
            | CoreRecordKind::AuditEvent
            | CoreRecordKind::ApprovalDecision
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInitRequest {
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub idempotency_key: String,
    pub payload_summary: String,
}

impl WorkspaceInitRequest {
    pub fn new(
        event_id: impl Into<String>,
        actor_id: impl Into<String>,
        workspace_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload_summary: impl Into<String>,
    ) -> Self {
        let workspace_id = workspace_id.into();

        Self {
            event_id: event_id.into(),
            schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            payload_schema_hash: "workspace.init.v1".to_string(),
            actor_id: actor_id.into(),
            scope: workspace_scope(&workspace_id),
            workspace_id,
            idempotency_key: idempotency_key.into(),
            payload_summary: payload_summary.into(),
        }
    }

    fn into_event(self) -> InertEvent {
        InertEvent {
            event_id: self.event_id,
            schema_version: self.schema_version,
            payload_schema_hash: self.payload_schema_hash,
            actor_id: self.actor_id,
            scope: self.scope,
            workspace_id: self.workspace_id,
            idempotency_key: self.idempotency_key,
            event_type: "workspace.init".to_string(),
            payload_summary: self.payload_summary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInitResult {
    pub workspace_id: String,
    pub workspace_revision: u64,
    pub audit_head: String,
    pub index_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQueryRequest {
    pub workspace_id: String,
    pub account_id: String,
    pub query: String,
    pub limit: usize,
}

impl SearchQueryRequest {
    pub fn new(
        workspace_id: impl Into<String>,
        account_id: impl Into<String>,
        query: impl Into<String>,
        limit: usize,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            account_id: account_id.into(),
            query: query.into(),
            limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStatusDTO {
    pub state: String,
    pub last_good_revision: Option<String>,
    pub stale_reason: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRangeDTO {
    pub unit: String,
    pub start: u64,
    pub end: u64,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRefDTO {
    pub citation_id: String,
    pub source_id: String,
    pub range: SourceRangeDTO,
    pub wiki_page_id: String,
    pub claim_id: String,
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultDTO {
    pub result_id: String,
    pub kind: String,
    pub title: String,
    pub snippet: Option<String>,
    pub citation_refs: Vec<CitationRefDTO>,
    pub index_status: IndexStatusDTO,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationResolveRequest {
    pub workspace_id: String,
    pub account_id: String,
    pub citation_id: String,
}

impl CitationResolveRequest {
    pub fn new(
        workspace_id: impl Into<String>,
        account_id: impl Into<String>,
        citation_id: impl Into<String>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            account_id: account_id.into(),
            citation_id: citation_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationResolveResult {
    pub citation_id: String,
    pub source_id: String,
    pub range: SourceRangeDTO,
    pub wiki_page_id: String,
    pub claim_id: String,
    pub preview_target: String,
    pub degraded_reason: Option<String>,
    pub source_card: Option<SourceCardDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCardDTO {
    pub source_id: String,
    pub title: String,
    pub origin_display: String,
    pub range: SourceRangeDTO,
    pub summary: String,
    pub visibility: String,
    pub citation_refs: Vec<CitationRefDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerComposerRequest {
    pub workspace_id: String,
    pub account_id: String,
    pub query: String,
    pub candidate_ids: Vec<String>,
}

impl AnswerComposerRequest {
    pub fn new(
        workspace_id: impl Into<String>,
        account_id: impl Into<String>,
        query: impl Into<String>,
        candidate_ids: Vec<String>,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            account_id: account_id.into(),
            query: query.into(),
            candidate_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerDTO {
    pub answer_id: String,
    pub text: String,
    pub citation_refs: Vec<CitationRefDTO>,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecisionStatus {
    Approved,
    Denied,
}

impl ApprovalDecisionStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => APPROVAL_DECISION_APPROVED,
            Self::Denied => APPROVAL_DECISION_DENIED,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecisionRecord {
    pub approval_id: String,
    pub patch_id: String,
    pub decision: ApprovalDecisionStatus,
    pub decided_by: String,
    pub reason_present: bool,
    pub reason_summary: Option<String>,
}

impl ApprovalDecisionRecord {
    pub fn new(
        approval_id: impl Into<String>,
        patch_id: impl Into<String>,
        decision: ApprovalDecisionStatus,
        decided_by: impl Into<String>,
        reason: Option<String>,
    ) -> Self {
        Self {
            approval_id: approval_id.into(),
            patch_id: patch_id.into(),
            decision,
            decided_by: decided_by.into(),
            reason_present: reason.is_some(),
            reason_summary: reason.and_then(|value| sanitized_reason_summary(&value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecisionRequest {
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub idempotency_key: String,
    pub record: ApprovalDecisionRecord,
}

impl ApprovalDecisionRequest {
    pub fn new(
        event_id: impl Into<String>,
        actor_id: impl Into<String>,
        workspace_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        record: ApprovalDecisionRecord,
    ) -> Self {
        let workspace_id = workspace_id.into();

        Self {
            event_id: event_id.into(),
            schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            payload_schema_hash: expected_payload_schema_hash(APPROVAL_DECISION_EVENT_TYPE),
            actor_id: actor_id.into(),
            scope: workspace_scope(&workspace_id),
            workspace_id,
            idempotency_key: idempotency_key.into(),
            record,
        }
    }

    fn into_event_and_record(self) -> (InertEvent, ApprovalDecisionRecord) {
        let payload_summary = approval_decision_payload_summary(&self.record);

        (
            InertEvent {
                event_id: self.event_id,
                schema_version: self.schema_version,
                payload_schema_hash: self.payload_schema_hash,
                actor_id: self.actor_id,
                scope: self.scope,
                workspace_id: self.workspace_id,
                idempotency_key: self.idempotency_key,
                event_type: APPROVAL_DECISION_EVENT_TYPE.to_string(),
                payload_summary,
            },
            self.record,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPatchCommitRequest {
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub idempotency_key: String,
    pub transaction_id: String,
    pub patch_id: String,
    pub approval_id: String,
    pub committed_revision: u64,
    pub rollback_marker_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProposeRequest {
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub idempotency_key: String,
    pub note_id: String,
    pub title: String,
    pub content: String,
}

impl MemoryProposeRequest {
    pub fn new(
        event_id: impl Into<String>,
        actor_id: impl Into<String>,
        workspace_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        note_id: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let workspace_id = workspace_id.into();
        Self {
            event_id: event_id.into(),
            schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            payload_schema_hash: expected_payload_schema_hash(MEMORY_PROPOSE_EVENT_TYPE),
            actor_id: actor_id.into(),
            scope: workspace_scope(&workspace_id),
            workspace_id,
            idempotency_key: idempotency_key.into(),
            note_id: note_id.into(),
            title: title.into(),
            content: content.into(),
        }
    }

    fn into_event(self) -> InertEvent {
        InertEvent {
            event_id: self.event_id,
            schema_version: self.schema_version,
            payload_schema_hash: self.payload_schema_hash,
            actor_id: self.actor_id,
            scope: self.scope,
            workspace_id: self.workspace_id,
            idempotency_key: self.idempotency_key,
            event_type: MEMORY_PROPOSE_EVENT_TYPE.to_string(),
            payload_summary: format!("note_id={} title={}", self.note_id, self.title),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCommitRequest {
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub idempotency_key: String,
    pub note_id: String,
    pub approval_id: String,
    pub committed_revision: u64,
}

impl MemoryCommitRequest {
    pub fn new(
        event_id: impl Into<String>,
        actor_id: impl Into<String>,
        workspace_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        note_id: impl Into<String>,
        approval_id: impl Into<String>,
        committed_revision: u64,
    ) -> Self {
        let workspace_id = workspace_id.into();
        Self {
            event_id: event_id.into(),
            schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            payload_schema_hash: expected_payload_schema_hash(MEMORY_COMMIT_EVENT_TYPE),
            actor_id: actor_id.into(),
            scope: workspace_scope(&workspace_id),
            workspace_id,
            idempotency_key: idempotency_key.into(),
            note_id: note_id.into(),
            approval_id: approval_id.into(),
            committed_revision,
        }
    }

    fn into_event(self) -> InertEvent {
        InertEvent {
            event_id: self.event_id,
            schema_version: self.schema_version,
            payload_schema_hash: self.payload_schema_hash,
            actor_id: self.actor_id,
            scope: self.scope,
            workspace_id: self.workspace_id,
            idempotency_key: self.idempotency_key,
            event_type: MEMORY_COMMIT_EVENT_TYPE.to_string(),
            payload_summary: format!(
                "note_id={} approval_id={} committed_revision={}",
                self.note_id, self.approval_id, self.committed_revision
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryNoteRecord {
    pub note_id: String,
    pub workspace_id: String,
    pub title: String,
    pub content: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPatchCommitRecord {
    pub transaction_id: String,
    pub patch_id: String,
    pub approval_id: String,
    pub committed_revision: u64,
    pub rollback_marker_id: String,
}

impl WikiPatchCommitRecord {
    pub fn new(
        transaction_id: impl Into<String>,
        patch_id: impl Into<String>,
        approval_id: impl Into<String>,
        committed_revision: u64,
        rollback_marker_id: impl Into<String>,
    ) -> Self {
        Self {
            transaction_id: transaction_id.into(),
            patch_id: patch_id.into(),
            approval_id: approval_id.into(),
            committed_revision,
            rollback_marker_id: rollback_marker_id.into(),
        }
    }
}

impl WikiPatchCommitRequest {
    pub fn new(
        event_id: impl Into<String>,
        actor_id: impl Into<String>,
        workspace_id: impl Into<String>,
        idempotency_key: impl Into<String>,
        commit: WikiPatchCommitRecord,
    ) -> Self {
        let workspace_id = workspace_id.into();

        Self {
            event_id: event_id.into(),
            schema_version: CURRENT_EVENT_SCHEMA_VERSION,
            payload_schema_hash: expected_payload_schema_hash(WIKI_PATCH_COMMIT_EVENT_TYPE),
            actor_id: actor_id.into(),
            scope: workspace_scope(&workspace_id),
            workspace_id,
            idempotency_key: idempotency_key.into(),
            transaction_id: commit.transaction_id,
            patch_id: commit.patch_id,
            approval_id: commit.approval_id,
            committed_revision: commit.committed_revision,
            rollback_marker_id: commit.rollback_marker_id,
        }
    }

    fn into_event(self) -> InertEvent {
        InertEvent {
            event_id: self.event_id,
            schema_version: self.schema_version,
            payload_schema_hash: self.payload_schema_hash,
            actor_id: self.actor_id,
            scope: self.scope,
            workspace_id: self.workspace_id,
            idempotency_key: self.idempotency_key,
            event_type: WIKI_PATCH_COMMIT_EVENT_TYPE.to_string(),
            payload_summary: format!(
                "transaction_id={} patch_id={} approval_id={} committed_revision={} rollback_marker_id={}",
                self.transaction_id,
                self.patch_id,
                self.approval_id,
                self.committed_revision,
                self.rollback_marker_id
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InertEvent {
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor_id: String,
    pub scope: String,
    pub workspace_id: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub payload_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEnvelope {
    pub seq: u64,
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor_id: String,
    pub scope: String,
    pub workspace_id: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub payload_summary: String,
}

impl From<(u64, InertEvent)> for EventEnvelope {
    fn from((seq, event): (u64, InertEvent)) -> Self {
        Self {
            seq,
            event_id: event.event_id,
            schema_version: event.schema_version,
            payload_schema_hash: event.payload_schema_hash,
            actor_id: event.actor_id,
            scope: event.scope,
            workspace_id: event.workspace_id,
            idempotency_key: event.idempotency_key,
            event_type: event.event_type,
            payload_summary: event.payload_summary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub audit_seq: u64,
    pub event_seq: u64,
    pub previous_hash: String,
    pub hash: String,
    pub event_digest: String,
}

#[derive(Debug)]
pub enum CoreError {
    Database(rusqlite::Error),
    Index(seaki_index::IndexError),
    InvalidSchemaVersion {
        found: u32,
    },
    InvalidPayloadSchemaHash {
        expected: String,
        found: String,
    },
    InvalidScope {
        expected: String,
        found: String,
    },
    EmptyIdempotencyKey,
    DuplicateIdempotencyKey(String),
    DuplicateEventId(String),
    DuplicateApprovalDecision(String),
    ApprovalDecisionRequired {
        approval_id: String,
        patch_id: String,
    },
    ApprovalDecisionNotApproved {
        approval_id: String,
        patch_id: String,
        decision: ApprovalDecisionStatus,
    },
    ApprovalDecisionPatchMismatch {
        approval_id: String,
        expected_patch_id: String,
        actual_patch_id: String,
    },
    ApprovalDecisionScopeMismatch {
        approval_id: String,
        expected_workspace_id: String,
        actual_workspace_id: String,
    },
    InvalidApprovalDecisionStatus(String),
    WorkspaceAlreadyExists(String),
    WorkspaceMissing(String),
    WorkspaceRevisionMismatch {
        expected: u64,
        found: u64,
    },
    AuditMissingForEvent(u64),
    SequenceOutOfRange(i64),
    PipelineCompose(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "database error: {error}"),
            Self::Index(error) => write!(f, "index error: {error}"),
            Self::InvalidSchemaVersion { found } => {
                write!(f, "invalid schema version: {found}")
            }
            Self::InvalidPayloadSchemaHash { expected, found } => {
                write!(
                    f,
                    "invalid payload schema hash: expected {expected}, found {found}"
                )
            }
            Self::InvalidScope { expected, found } => {
                write!(f, "invalid scope: expected {expected}, found {found}")
            }
            Self::EmptyIdempotencyKey => write!(f, "idempotency key must not be empty"),
            Self::DuplicateIdempotencyKey(key) => {
                write!(f, "duplicate idempotency key: {key}")
            }
            Self::DuplicateEventId(event_id) => write!(f, "duplicate event id: {event_id}"),
            Self::DuplicateApprovalDecision(approval_id) => {
                write!(f, "duplicate approval decision: {approval_id}")
            }
            Self::ApprovalDecisionRequired {
                approval_id,
                patch_id,
            } => write!(
                f,
                "approval decision {approval_id} is required before committing patch {patch_id}"
            ),
            Self::ApprovalDecisionNotApproved {
                approval_id,
                patch_id,
                decision,
            } => write!(
                f,
                "approval decision {approval_id} for patch {patch_id} is not approved: {decision:?}"
            ),
            Self::ApprovalDecisionPatchMismatch {
                approval_id,
                expected_patch_id,
                actual_patch_id,
            } => write!(
                f,
                "approval decision {approval_id} targets patch {actual_patch_id}, expected {expected_patch_id}"
            ),
            Self::ApprovalDecisionScopeMismatch {
                approval_id,
                expected_workspace_id,
                actual_workspace_id,
            } => write!(
                f,
                "approval decision {approval_id} workspace mismatch: expected {expected_workspace_id}, got {actual_workspace_id}"
            ),
            Self::InvalidApprovalDecisionStatus(status) => {
                write!(f, "invalid approval decision status: {status}")
            }
            Self::WorkspaceAlreadyExists(workspace_id) => {
                write!(f, "workspace already exists: {workspace_id}")
            }
            Self::WorkspaceMissing(workspace_id) => write!(f, "workspace missing: {workspace_id}"),
            Self::WorkspaceRevisionMismatch { expected, found } => {
                write!(
                    f,
                    "workspace revision mismatch: expected {expected}, found {found}"
                )
            }
            Self::AuditMissingForEvent(seq) => write!(f, "audit missing for event seq {seq}"),
            Self::SequenceOutOfRange(seq) => write!(f, "sequence out of range: {seq}"),
            Self::PipelineCompose(reason) => write!(f, "pipeline compose error: {reason}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<rusqlite::Error> for CoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<seaki_index::IndexError> for CoreError {
    fn from(value: seaki_index::IndexError) -> Self {
        Self::Index(value)
    }
}

pub type CoreResult<T> = Result<T, CoreError>;

pub struct CoreLedger {
    conn: Connection,
    search_index: Bm25CandidateIndex,
}

impl CoreLedger {
    pub fn open(path: impl AsRef<Path>) -> CoreResult<Self> {
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    pub fn open_in_memory() -> CoreResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::from_connection(conn)
    }

    pub fn from_connection(conn: Connection) -> CoreResult<Self> {
        let mut ledger = Self {
            conn,
            search_index: Bm25CandidateIndex::new(),
        };
        ledger.initialize()?;
        Ok(ledger)
    }

    fn initialize(&mut self) -> CoreResult<()> {
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS workspaces (
                workspace_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL CHECK (revision >= 0),
                audit_head TEXT NOT NULL,
                index_status TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                schema_version INTEGER NOT NULL,
                payload_schema_hash TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                workspace_id TEXT NOT NULL,
                idempotency_key TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                payload_summary TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit (
                audit_seq INTEGER PRIMARY KEY AUTOINCREMENT,
                event_seq INTEGER NOT NULL UNIQUE REFERENCES events(seq),
                previous_hash TEXT NOT NULL,
                hash TEXT NOT NULL,
                event_digest TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS approval_decisions (
                approval_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                patch_id TEXT NOT NULL,
                decision TEXT NOT NULL CHECK (decision IN ('approved', 'denied')),
                decided_by TEXT NOT NULL,
                reason_present INTEGER NOT NULL CHECK (reason_present IN (0, 1)),
                reason_summary TEXT,
                event_seq INTEGER NOT NULL UNIQUE REFERENCES events(seq)
            );

            CREATE INDEX IF NOT EXISTS idx_approval_decisions_patch_id
                ON approval_decisions (patch_id);

            CREATE TABLE IF NOT EXISTS memory_notes (
                note_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                status TEXT NOT NULL,
                event_seq INTEGER NOT NULL REFERENCES events(seq)
            );
            ",
        )?;
        Ok(())
    }

    pub fn journal_mode(&self) -> CoreResult<String> {
        Ok(self
            .conn
            .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?)
    }

    pub fn workspace_init(
        &mut self,
        request: WorkspaceInitRequest,
    ) -> CoreResult<WorkspaceInitResult> {
        let event = request.into_event();
        validate_event(&event)?;
        self.ensure_unique_idempotency_key(&event.idempotency_key)?;
        self.ensure_unique_event_id(&event.event_id)?;
        self.ensure_workspace_absent(&event.workspace_id)?;

        let tx = self.conn.transaction()?;
        let sanitized_event = sanitized_event(event);
        insert_workspace(&tx, &sanitized_event.workspace_id, GENESIS_AUDIT_HASH)?;
        let envelope = insert_event(&tx, sanitized_event)?;
        let audit_head = append_audit_entry(&tx, &envelope)?;
        update_workspace_after_event(&tx, &envelope.workspace_id, 1, &audit_head)?;
        tx.commit()?;

        Ok(WorkspaceInitResult {
            workspace_id: envelope.workspace_id,
            workspace_revision: 1,
            audit_head,
            index_status: INDEX_STATUS_STALE.to_string(),
        })
    }

    pub fn replace_search_scope(
        &mut self,
        generation: IndexGeneration,
        documents: impl IntoIterator<Item = IndexedDocument>,
    ) -> CoreResult<()> {
        let scope = generation.scope();
        self.workspace_revision(&scope.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(scope.workspace_id.clone()))?;
        self.search_index.replace_scope(generation, documents)?;
        Ok(())
    }

    pub fn search_query(&self, request: SearchQueryRequest) -> CoreResult<Vec<SearchResultDTO>> {
        self.workspace_revision(&request.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(request.workspace_id.clone()))?;

        let query = seaki_index::SearchQuery::new(
            request.workspace_id,
            request.account_id,
            request.query,
            request.limit,
        );
        let candidate_search = self.search_index.search_candidates(&query);
        let generation = self.search_index.generation(&query.scope());
        let index_status = search_index_status(candidate_search.status, generation);
        let authorized = self
            .search_index
            .authorize_candidates(&query, &candidate_search.candidate_ids);

        Ok(authorized
            .into_iter()
            .map(|result| SearchResultDTO {
                result_id: result.candidate_id.to_string(),
                kind: search_result_kind(&result.kind).to_string(),
                title: result.title,
                snippet: result.snippet,
                citation_refs: result
                    .citation_refs
                    .into_iter()
                    .map(CitationRefDTO::from)
                    .collect(),
                index_status: index_status.clone(),
            })
            .collect())
    }

    pub fn citation_resolve(
        &self,
        request: CitationResolveRequest,
    ) -> CoreResult<CitationResolveResult> {
        self.workspace_revision(&request.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(request.workspace_id.clone()))?;

        let scope = seaki_index::IndexScope::new(request.workspace_id, request.account_id);
        let resolved = self
            .search_index
            .resolve_citation(&scope, &request.citation_id);

        let Some(result) = resolved else {
            return Ok(CitationResolveResult {
                citation_id: request.citation_id.clone(),
                source_id: String::new(),
                range: SourceRangeDTO {
                    unit: "line".to_string(),
                    start: 0,
                    end: 0,
                    label: None,
                },
                wiki_page_id: String::new(),
                claim_id: String::new(),
                preview_target: "none".to_string(),
                degraded_reason: Some("citation not found or not visible".to_string()),
                source_card: None,
            });
        };

        let citation_ref = result.citation_refs.first().cloned();
        let degraded = citation_ref
            .as_ref()
            .and_then(|c| c.degraded_reason.clone());

        let source_card = if degraded.is_some() {
            None
        } else {
            Some(SourceCardDTO {
                source_id: result.source_id.clone(),
                title: result.title.clone(),
                origin_display: result.title.clone(),
                range: SourceRangeDTO {
                    unit: "line".to_string(),
                    start: 0,
                    end: 0,
                    label: None,
                },
                summary: result.snippet.unwrap_or_default(),
                visibility: "visible".to_string(),
                citation_refs: result
                    .citation_refs
                    .into_iter()
                    .map(CitationRefDTO::from)
                    .collect(),
            })
        };

        let preview_target = if degraded.is_some() {
            "none"
        } else {
            "source_range"
        }
        .to_string();

        let citation_ref = citation_ref.map_or_else(
            || CitationRefDTO {
                citation_id: request.citation_id.clone(),
                source_id: result.source_id.clone(),
                range: SourceRangeDTO {
                    unit: "line".to_string(),
                    start: 0,
                    end: 0,
                    label: None,
                },
                wiki_page_id: String::new(),
                claim_id: String::new(),
                degraded_reason: degraded.clone(),
            },
            CitationRefDTO::from,
        );

        Ok(CitationResolveResult {
            citation_id: citation_ref.citation_id.clone(),
            source_id: citation_ref.source_id.clone(),
            range: citation_ref.range.clone(),
            wiki_page_id: citation_ref.wiki_page_id.clone(),
            claim_id: citation_ref.claim_id.clone(),
            preview_target,
            degraded_reason: degraded,
            source_card,
        })
    }

    pub fn compose_answer(&self, request: AnswerComposerRequest) -> CoreResult<AnswerDTO> {
        self.workspace_revision(&request.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(request.workspace_id.clone()))?;

        let query = seaki_index::SearchQuery::new(
            request.workspace_id.clone(),
            request.account_id.clone(),
            request.query.clone(),
            request.candidate_ids.len().max(10),
        );

        let candidate_search = self.search_index.search_candidates(&query);
        let candidate_ids = if request.candidate_ids.is_empty() {
            candidate_search.candidate_ids.clone()
        } else {
            let allowed: std::collections::HashSet<_> =
                request.candidate_ids.iter().cloned().collect();
            candidate_search
                .candidate_ids
                .into_iter()
                .filter(|id| allowed.contains(&id.0))
                .collect()
        };
        let authorized = self
            .search_index
            .authorize_candidates(&query, &candidate_ids);

        let mut citation_refs = Vec::new();
        let mut answer_text = String::new();

        for result in authorized {
            if result.snippet.is_none() {
                continue;
            }
            for citation_ref in &result.citation_refs {
                if citation_ref.degraded_reason.is_some() {
                    continue;
                }
                citation_refs.push(CitationRefDTO::from(citation_ref.clone()));
            }
            if !answer_text.is_empty() {
                answer_text.push('\n');
            }
            answer_text.push_str(&result.title);
            answer_text.push_str(": ");
            answer_text.push_str(result.snippet.as_deref().unwrap_or(""));
        }

        let status = if citation_refs.is_empty() {
            "no_access"
        } else if candidate_search.status != seaki_index::IndexStatus::Fresh {
            "degraded"
        } else {
            "composed"
        };

        Ok(AnswerDTO {
            answer_id: format!("answer-{}-{}", request.workspace_id, request.query),
            text: answer_text,
            citation_refs,
            status: status.to_string(),
        })
    }

    pub fn pipe_list(
        &self,
        filter: Option<seaki_pipe::SideEffectFilter>,
    ) -> Vec<seaki_pipe::PipeCommandSummary> {
        let registry = seaki_pipe::CommandRegistry::builtin();
        let manifests = registry.list();
        manifests
            .into_iter()
            .filter(|m| match &filter {
                Some(seaki_pipe::SideEffectFilter::Level(level)) => m.side_effect_level == *level,
                _ => true,
            })
            .map(|m| seaki_pipe::PipeCommandSummary {
                command_id: m.command_id.clone(),
                description: m.description.clone(),
                side_effect_level: m.side_effect_level.to_string(),
            })
            .collect()
    }

    pub fn pipe_inspect(
        &self,
        command_id: &str,
    ) -> Result<seaki_pipe::PipeCommandManifest, seaki_pipe::CommandNotFound> {
        let registry = seaki_pipe::CommandRegistry::builtin();
        registry.inspect(command_id).cloned()
    }

    pub fn pipe_dry_run(
        &self,
        ast: seaki_pipe::PipelineAst,
        initial_input: serde_json::Value,
    ) -> CoreResult<seaki_pipe::DryRunResult> {
        let registry = seaki_pipe::CommandRegistry::builtin();
        let composed = seaki_pipe::compose(&ast, &registry)
            .map_err(|e| CoreError::PipelineCompose(e.to_string()))?;
        Ok(seaki_pipe::dry_run(&composed, initial_input))
    }

    pub fn append_inert_event(&mut self, event: InertEvent) -> CoreResult<EventEnvelope> {
        validate_event(&event)?;
        self.ensure_unique_idempotency_key(&event.idempotency_key)?;
        self.ensure_unique_event_id(&event.event_id)?;
        let revision = self
            .workspace_revision(&event.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(event.workspace_id.clone()))?
            + 1;

        let tx = self.conn.transaction()?;
        let envelope = insert_event(&tx, sanitized_event(event))?;
        let audit_head = append_audit_entry(&tx, &envelope)?;
        update_workspace_after_event(&tx, &envelope.workspace_id, revision, &audit_head)?;
        tx.commit()?;

        Ok(envelope)
    }

    pub fn append_memory_propose(
        &mut self,
        request: MemoryProposeRequest,
    ) -> CoreResult<EventEnvelope> {
        let note_id = request.note_id.clone();
        let workspace_id = request.workspace_id.clone();
        let title = request.title.clone();
        let content = request.content.clone();
        let event = request.into_event();
        validate_event(&event)?;
        self.ensure_unique_idempotency_key(&event.idempotency_key)?;
        self.ensure_unique_event_id(&event.event_id)?;
        let revision = self
            .workspace_revision(&event.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(event.workspace_id.clone()))?
            + 1;

        let tx = self.conn.transaction()?;
        let envelope = insert_event(&tx, sanitized_event(event))?;
        insert_memory_note(&tx, &note_id, &workspace_id, &title, &content, envelope.seq)?;
        let audit_head = append_audit_entry(&tx, &envelope)?;
        update_workspace_after_event(&tx, &envelope.workspace_id, revision, &audit_head)?;
        tx.commit()?;

        Ok(envelope)
    }

    pub fn append_memory_commit(
        &mut self,
        request: MemoryCommitRequest,
    ) -> CoreResult<EventEnvelope> {
        let approval_id = request.approval_id.clone();
        let note_id = request.note_id.clone();
        let committed_revision = request.committed_revision;
        let event = request.into_event();
        validate_event(&event)?;

        let tx = self.conn.transaction()?;
        ensure_unique_idempotency_key_in_tx(&tx, &event.idempotency_key)?;
        ensure_unique_event_id_in_tx(&tx, &event.event_id)?;
        let expected_revision = workspace_revision_in_tx(&tx, &event.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(event.workspace_id.clone()))?
            + 1;
        ensure_approved_decision_for_target(&tx, &approval_id, &note_id, &event.workspace_id)?;
        if committed_revision != expected_revision {
            return Err(CoreError::WorkspaceRevisionMismatch {
                expected: expected_revision,
                found: committed_revision,
            });
        }

        let envelope = insert_event(&tx, sanitized_event(event))?;
        update_memory_note_status(&tx, &note_id, "active")?;
        let audit_head = append_audit_entry(&tx, &envelope)?;
        update_workspace_after_event(&tx, &envelope.workspace_id, expected_revision, &audit_head)?;
        tx.commit()?;

        Ok(envelope)
    }

    pub fn memory_note(&self, note_id: &str) -> CoreResult<Option<MemoryNoteRecord>> {
        self.conn
            .query_row(
                "SELECT note_id, workspace_id, title, content, created_at, updated_at, status
                 FROM memory_notes WHERE note_id = ?1",
                params![note_id],
                |row| {
                    Ok(MemoryNoteRecord {
                        note_id: row.get(0)?,
                        workspace_id: row.get(1)?,
                        title: row.get(2)?,
                        content: row.get(3)?,
                        created_at: checked_u64(row.get::<_, i64>(4)?)?,
                        updated_at: checked_u64(row.get::<_, i64>(5)?)?,
                        status: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(CoreError::from)
    }

    pub fn memory_source_check(
        &mut self,
        note_id: &str,
        wiki_claim_keywords: &[String],
    ) -> CoreResult<MemoryNoteRecord> {
        let tx = self.conn.transaction()?;
        let Some(record) = memory_note_in_tx(&tx, note_id)? else {
            return Err(CoreError::WorkspaceMissing(format!(
                "note not found: {note_id}"
            )));
        };

        let content_lower = record.content.to_lowercase();
        let has_conflict = wiki_claim_keywords
            .iter()
            .any(|kw| content_lower.contains(&kw.to_lowercase()));

        let new_status = if has_conflict {
            "conflict"
        } else {
            "source_checking"
        };
        update_memory_note_status(&tx, note_id, new_status)?;
        tx.commit()?;

        self.memory_note(note_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(format!("note not found: {note_id}")))
    }

    pub fn append_wiki_patch_commit(
        &mut self,
        request: WikiPatchCommitRequest,
    ) -> CoreResult<EventEnvelope> {
        let approval_id = request.approval_id.clone();
        let patch_id = request.patch_id.clone();
        let committed_revision = request.committed_revision;
        let event = request.into_event();
        validate_event(&event)?;

        let tx = self.conn.transaction()?;
        ensure_unique_idempotency_key_in_tx(&tx, &event.idempotency_key)?;
        ensure_unique_event_id_in_tx(&tx, &event.event_id)?;
        let expected_revision = workspace_revision_in_tx(&tx, &event.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(event.workspace_id.clone()))?
            + 1;
        ensure_approved_decision_for_commit(&tx, &approval_id, &patch_id, &event.workspace_id)?;
        if committed_revision != expected_revision {
            return Err(CoreError::WorkspaceRevisionMismatch {
                expected: expected_revision,
                found: committed_revision,
            });
        }

        let envelope = insert_event(&tx, sanitized_event(event))?;
        let audit_head = append_audit_entry(&tx, &envelope)?;
        update_workspace_after_event(&tx, &envelope.workspace_id, expected_revision, &audit_head)?;
        tx.commit()?;

        Ok(envelope)
    }

    pub fn append_approval_decision(
        &mut self,
        request: ApprovalDecisionRequest,
    ) -> CoreResult<EventEnvelope> {
        let (event, record) = request.into_event_and_record();
        validate_event(&event)?;

        let tx = self.conn.transaction()?;
        ensure_unique_idempotency_key_in_tx(&tx, &event.idempotency_key)?;
        ensure_unique_event_id_in_tx(&tx, &event.event_id)?;
        ensure_approval_decision_absent_in_tx(&tx, &record.approval_id)?;
        let revision = workspace_revision_in_tx(&tx, &event.workspace_id)?
            .ok_or_else(|| CoreError::WorkspaceMissing(event.workspace_id.clone()))?
            + 1;

        let envelope = insert_event(&tx, sanitized_event(event))?;
        insert_approval_decision(&tx, &record, &envelope.workspace_id, envelope.seq)?;
        let audit_head = append_audit_entry(&tx, &envelope)?;
        update_workspace_after_event(&tx, &envelope.workspace_id, revision, &audit_head)?;
        tx.commit()?;

        Ok(envelope)
    }

    pub fn replay_events_after(&self, seq: u64) -> CoreResult<Vec<EventEnvelope>> {
        let seq = checked_i64(seq)?;
        let mut stmt = self.conn.prepare(
            "
            SELECT
                seq,
                event_id,
                schema_version,
                payload_schema_hash,
                actor_id,
                scope,
                workspace_id,
                idempotency_key,
                event_type,
                payload_summary
            FROM events
            WHERE seq > ?1
            ORDER BY seq ASC
            ",
        )?;
        let rows = stmt.query_map(params![seq], read_event_envelope)?;

        rows.map(|row| row.map_err(CoreError::from)).collect()
    }

    pub fn audit_entries(&self) -> CoreResult<Vec<AuditEntry>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT audit_seq, event_seq, previous_hash, hash, event_digest
            FROM audit
            ORDER BY audit_seq ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AuditEntry {
                audit_seq: checked_u64(row.get::<_, i64>(0)?)?,
                event_seq: checked_u64(row.get::<_, i64>(1)?)?,
                previous_hash: row.get(2)?,
                hash: row.get(3)?,
                event_digest: row.get(4)?,
            })
        })?;

        rows.map(|row| row.map_err(CoreError::from)).collect()
    }

    pub fn verify_audit_chain(&self) -> CoreResult<bool> {
        let events = self.replay_events_after(0)?;
        let audit_entries = self.audit_entries()?;

        if events.len() != audit_entries.len() {
            return Ok(false);
        }

        let mut previous_hash = GENESIS_AUDIT_HASH.to_string();
        for (event, audit_entry) in events.iter().zip(audit_entries.iter()) {
            let event_digest = event_digest(event);
            let hash = audit_hash(&previous_hash, &event_digest);

            if audit_entry.event_seq != event.seq
                || audit_entry.previous_hash != previous_hash
                || audit_entry.event_digest != event_digest
                || audit_entry.hash != hash
            {
                return Ok(false);
            }

            previous_hash.clone_from(&audit_entry.hash);
        }

        Ok(true)
    }

    pub fn event_count(&self) -> CoreResult<u64> {
        let count = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| {
                row.get::<_, i64>(0)
            })?;
        checked_u64(count).map_err(CoreError::from)
    }

    pub fn audit_count(&self) -> CoreResult<u64> {
        let count = self
            .conn
            .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get::<_, i64>(0))?;
        checked_u64(count).map_err(CoreError::from)
    }

    pub fn workspace_revision(&self, workspace_id: &str) -> CoreResult<Option<u64>> {
        let revision = self
            .conn
            .query_row(
                "SELECT revision FROM workspaces WHERE workspace_id = ?1",
                params![workspace_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;

        revision
            .map(checked_u64)
            .transpose()
            .map_err(CoreError::from)
    }

    pub fn audit_head(&self, workspace_id: &str) -> CoreResult<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT audit_head FROM workspaces WHERE workspace_id = ?1",
                params![workspace_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }

    pub fn index_status(&self, workspace_id: &str) -> CoreResult<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT index_status FROM workspaces WHERE workspace_id = ?1",
                params![workspace_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }

    pub fn approval_decision(
        &self,
        approval_id: &str,
    ) -> CoreResult<Option<ApprovalDecisionRecord>> {
        self.conn
            .query_row(
                "
                SELECT approval_id, patch_id, decision, decided_by, reason_present, reason_summary
                FROM approval_decisions
                WHERE approval_id = ?1
                ",
                params![approval_id],
                read_approval_decision_record,
            )
            .optional()
            .map_err(CoreError::from)
    }

    fn ensure_unique_idempotency_key(&self, idempotency_key: &str) -> CoreResult<()> {
        let exists: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM events WHERE idempotency_key = ?1",
                params![idempotency_key],
                |row| row.get(0),
            )
            .optional()?;

        if exists.is_some() {
            return Err(CoreError::DuplicateIdempotencyKey(
                idempotency_key.to_string(),
            ));
        }

        Ok(())
    }

    fn ensure_unique_event_id(&self, event_id: &str) -> CoreResult<()> {
        let exists: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM events WHERE event_id = ?1",
                params![event_id],
                |row| row.get(0),
            )
            .optional()?;

        if exists.is_some() {
            return Err(CoreError::DuplicateEventId(event_id.to_string()));
        }

        Ok(())
    }

    fn ensure_workspace_absent(&self, workspace_id: &str) -> CoreResult<()> {
        if self.workspace_revision(workspace_id)?.is_some() {
            return Err(CoreError::WorkspaceAlreadyExists(workspace_id.to_string()));
        }

        Ok(())
    }
}

#[must_use]
pub fn workspace_scope(workspace_id: &str) -> String {
    format!("workspace:{workspace_id}")
}

impl From<IndexedCitationRef> for CitationRefDTO {
    fn from(value: IndexedCitationRef) -> Self {
        let range = value.range;
        Self {
            citation_id: value.citation_id,
            source_id: value.source_id,
            range: SourceRangeDTO {
                unit: source_range_unit(&range.unit).to_string(),
                start: range.start,
                end: range.end,
                label: range.label,
            },
            wiki_page_id: value.wiki_page_id,
            claim_id: value.claim_id,
            degraded_reason: value.degraded_reason,
        }
    }
}

fn search_index_status(
    status: IndexStatus,
    generation: Option<&IndexGeneration>,
) -> IndexStatusDTO {
    let state = match status {
        IndexStatus::Fresh => INDEX_STATUS_FRESH,
        IndexStatus::Stale | IndexStatus::CleanupRequired => INDEX_STATUS_STALE,
        IndexStatus::Failed => INDEX_STATUS_ERROR,
    };
    let stale_reason = match status {
        IndexStatus::Fresh => None,
        IndexStatus::Stale => Some("index stale".to_string()),
        IndexStatus::CleanupRequired => Some("index cleanup required".to_string()),
        IndexStatus::Failed => generation.and_then(|generation| generation.failure_reason.clone()),
    };

    IndexStatusDTO {
        state: state.to_string(),
        last_good_revision: generation.map(|generation| generation.wiki_revision.to_string()),
        stale_reason,
        updated_at: None,
    }
}

fn search_result_kind(kind: &CandidateKind) -> &'static str {
    match kind {
        CandidateKind::WikiPage => "wiki_page",
        CandidateKind::Claim => "claim",
        CandidateKind::SourceFrame => "source",
        CandidateKind::MemoryNote => "memory_note",
    }
}

fn source_range_unit(unit: &SourceRangeUnit) -> &'static str {
    match unit {
        SourceRangeUnit::Byte => "byte",
        SourceRangeUnit::Line => "line",
        SourceRangeUnit::Page => "page",
        SourceRangeUnit::Anchor => "anchor",
    }
}

fn validate_event(event: &InertEvent) -> CoreResult<()> {
    if event.schema_version != CURRENT_EVENT_SCHEMA_VERSION {
        return Err(CoreError::InvalidSchemaVersion {
            found: event.schema_version,
        });
    }

    let expected_payload_schema_hash = expected_payload_schema_hash(&event.event_type);
    if event.payload_schema_hash != expected_payload_schema_hash {
        return Err(CoreError::InvalidPayloadSchemaHash {
            expected: expected_payload_schema_hash,
            found: event.payload_schema_hash.clone(),
        });
    }

    let expected_scope = workspace_scope(&event.workspace_id);
    if event.scope != expected_scope {
        return Err(CoreError::InvalidScope {
            expected: expected_scope,
            found: event.scope.clone(),
        });
    }

    if event.idempotency_key.trim().is_empty() {
        return Err(CoreError::EmptyIdempotencyKey);
    }

    Ok(())
}

#[must_use]
pub fn expected_payload_schema_hash(event_type: &str) -> String {
    format!("{event_type}.v1")
}

fn approval_decision_payload_summary(record: &ApprovalDecisionRecord) -> String {
    let mut summary = format!(
        "approval_id={} patch_id={} decision={} decided_by={} reason_present={}",
        record.approval_id,
        record.patch_id,
        record.decision.as_str(),
        record.decided_by,
        record.reason_present
    );

    if let Some(reason_summary) = &record.reason_summary {
        summary.push_str(" reason_summary=");
        summary.push_str(reason_summary);
    }

    summary
}

fn sanitized_reason_summary(reason: &str) -> Option<String> {
    const MAX_REASON_SUMMARY_CHARS: usize = 96;

    let sanitized = sanitize_payload_summary(reason).trim().to_string();
    if sanitized.is_empty() {
        return None;
    }

    Some(sanitized.chars().take(MAX_REASON_SUMMARY_CHARS).collect())
}

fn sanitized_event(mut event: InertEvent) -> InertEvent {
    event.payload_summary = sanitize_payload_summary(&event.payload_summary);
    event
}

#[must_use]
pub fn sanitize_payload_summary(summary: &str) -> String {
    let mut redact_next = false;
    let mut sanitized = Vec::new();

    for token in summary.split_whitespace() {
        if redact_next {
            sanitized.push("[REDACTED]".to_string());
            redact_next = false;
            continue;
        }

        let (cleaned, should_redact_next) = sanitize_token(token);
        sanitized.push(cleaned);
        redact_next = should_redact_next;
    }

    sanitized.join(" ")
}

fn sanitize_token(token: &str) -> (String, bool) {
    let lower = token.to_ascii_lowercase();

    if lower == "bearer" {
        return ("[REDACTED]".to_string(), true);
    }

    if lower.contains("secret") || lower.contains("token") || lower.contains("bearer") {
        if let Some((key, _)) = token.split_once('=') {
            return (format!("{key}=[REDACTED]"), false);
        }

        if let Some((key, _)) = token.split_once(':') {
            return (format!("{key}:[REDACTED]"), false);
        }

        return ("[REDACTED]".to_string(), true);
    }

    (token.to_string(), false)
}

fn insert_workspace(tx: &Transaction<'_>, workspace_id: &str, audit_head: &str) -> CoreResult<()> {
    tx.execute(
        "
        INSERT INTO workspaces (workspace_id, revision, audit_head, index_status)
        VALUES (?1, 0, ?2, ?3)
        ",
        params![workspace_id, audit_head, INDEX_STATUS_STALE],
    )?;
    Ok(())
}

fn workspace_revision_in_tx(tx: &Transaction<'_>, workspace_id: &str) -> CoreResult<Option<u64>> {
    let revision = tx
        .query_row(
            "SELECT revision FROM workspaces WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;

    revision
        .map(checked_u64)
        .transpose()
        .map_err(CoreError::from)
}

fn ensure_unique_idempotency_key_in_tx(
    tx: &Transaction<'_>,
    idempotency_key: &str,
) -> CoreResult<()> {
    let exists: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM events WHERE idempotency_key = ?1",
            params![idempotency_key],
            |row| row.get(0),
        )
        .optional()?;

    if exists.is_some() {
        return Err(CoreError::DuplicateIdempotencyKey(
            idempotency_key.to_string(),
        ));
    }

    Ok(())
}

fn ensure_unique_event_id_in_tx(tx: &Transaction<'_>, event_id: &str) -> CoreResult<()> {
    let exists: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM events WHERE event_id = ?1",
            params![event_id],
            |row| row.get(0),
        )
        .optional()?;

    if exists.is_some() {
        return Err(CoreError::DuplicateEventId(event_id.to_string()));
    }

    Ok(())
}

fn ensure_approval_decision_absent_in_tx(
    tx: &Transaction<'_>,
    approval_id: &str,
) -> CoreResult<()> {
    let exists: Option<i64> = tx
        .query_row(
            "SELECT 1 FROM approval_decisions WHERE approval_id = ?1",
            params![approval_id],
            |row| row.get(0),
        )
        .optional()?;

    if exists.is_some() {
        return Err(CoreError::DuplicateApprovalDecision(
            approval_id.to_string(),
        ));
    }

    Ok(())
}

fn ensure_approved_decision_for_target(
    tx: &Transaction<'_>,
    approval_id: &str,
    target_id: &str,
    workspace_id: &str,
) -> CoreResult<()> {
    let Some((decision_workspace_id, decision)) = approval_decision_in_tx(tx, approval_id)? else {
        return Err(CoreError::ApprovalDecisionRequired {
            approval_id: approval_id.to_string(),
            patch_id: target_id.to_string(),
        });
    };

    if decision.patch_id != target_id {
        return Err(CoreError::ApprovalDecisionPatchMismatch {
            approval_id: approval_id.to_string(),
            expected_patch_id: target_id.to_string(),
            actual_patch_id: decision.patch_id,
        });
    }

    if decision_workspace_id != workspace_id {
        return Err(CoreError::ApprovalDecisionScopeMismatch {
            approval_id: approval_id.to_string(),
            expected_workspace_id: workspace_id.to_string(),
            actual_workspace_id: decision_workspace_id,
        });
    }

    if decision.decision != ApprovalDecisionStatus::Approved {
        return Err(CoreError::ApprovalDecisionNotApproved {
            approval_id: approval_id.to_string(),
            patch_id: target_id.to_string(),
            decision: decision.decision,
        });
    }

    Ok(())
}

fn ensure_approved_decision_for_commit(
    tx: &Transaction<'_>,
    approval_id: &str,
    patch_id: &str,
    workspace_id: &str,
) -> CoreResult<()> {
    ensure_approved_decision_for_target(tx, approval_id, patch_id, workspace_id)
}

fn approval_decision_in_tx(
    tx: &Transaction<'_>,
    approval_id: &str,
) -> CoreResult<Option<(String, ApprovalDecisionRecord)>> {
    tx.query_row(
        "
        SELECT
            approval_id,
            workspace_id,
            patch_id,
            decision,
            decided_by,
            reason_present,
            reason_summary
        FROM approval_decisions
        WHERE approval_id = ?1
        ",
        params![approval_id],
        |row| {
            let decision_status = row.get::<_, String>(3)?;
            let decision =
                approval_decision_status_from_str(&decision_status).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        3,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;

            Ok((
                row.get(1)?,
                ApprovalDecisionRecord {
                    approval_id: row.get(0)?,
                    patch_id: row.get(2)?,
                    decision,
                    decided_by: row.get(4)?,
                    reason_present: row.get::<_, i64>(5)? != 0,
                    reason_summary: row.get(6)?,
                },
            ))
        },
    )
    .optional()
    .map_err(CoreError::from)
}

fn insert_event(tx: &Transaction<'_>, event: InertEvent) -> CoreResult<EventEnvelope> {
    tx.execute(
        "
        INSERT INTO events (
            event_id,
            schema_version,
            payload_schema_hash,
            actor_id,
            scope,
            workspace_id,
            idempotency_key,
            event_type,
            payload_summary
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            event.event_id,
            event.schema_version,
            event.payload_schema_hash,
            event.actor_id,
            event.scope,
            event.workspace_id,
            event.idempotency_key,
            event.event_type,
            event.payload_summary
        ],
    )?;

    let seq = checked_u64(tx.last_insert_rowid())?;
    Ok((seq, event).into())
}

fn insert_approval_decision(
    tx: &Transaction<'_>,
    record: &ApprovalDecisionRecord,
    workspace_id: &str,
    event_seq: u64,
) -> CoreResult<()> {
    tx.execute(
        "
        INSERT INTO approval_decisions (
            approval_id,
            workspace_id,
            patch_id,
            decision,
            decided_by,
            reason_present,
            reason_summary,
            event_seq
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        params![
            &record.approval_id,
            workspace_id,
            &record.patch_id,
            record.decision.as_str(),
            &record.decided_by,
            i64::from(record.reason_present),
            &record.reason_summary,
            checked_i64(event_seq)?
        ],
    )?;
    Ok(())
}

fn append_audit_entry(tx: &Transaction<'_>, envelope: &EventEnvelope) -> CoreResult<String> {
    let previous_hash = tx
        .query_row(
            "SELECT hash FROM audit ORDER BY audit_seq DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| GENESIS_AUDIT_HASH.to_string());
    let event_digest = event_digest(envelope);
    let hash = audit_hash(&previous_hash, &event_digest);

    tx.execute(
        "
        INSERT INTO audit (event_seq, previous_hash, hash, event_digest)
        VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            checked_i64(envelope.seq)?,
            previous_hash,
            hash,
            event_digest
        ],
    )?;

    Ok(hash)
}

fn update_workspace_after_event(
    tx: &Transaction<'_>,
    workspace_id: &str,
    revision: u64,
    audit_head: &str,
) -> CoreResult<()> {
    tx.execute(
        "
        UPDATE workspaces
        SET revision = ?2, audit_head = ?3, index_status = ?4
        WHERE workspace_id = ?1
        ",
        params![
            workspace_id,
            checked_i64(revision)?,
            audit_head,
            INDEX_STATUS_STALE
        ],
    )?;
    Ok(())
}

fn read_event_envelope(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventEnvelope> {
    Ok(EventEnvelope {
        seq: checked_u64(row.get::<_, i64>(0)?)?,
        event_id: row.get(1)?,
        schema_version: row.get(2)?,
        payload_schema_hash: row.get(3)?,
        actor_id: row.get(4)?,
        scope: row.get(5)?,
        workspace_id: row.get(6)?,
        idempotency_key: row.get(7)?,
        event_type: row.get(8)?,
        payload_summary: row.get(9)?,
    })
}

fn read_approval_decision_record(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ApprovalDecisionRecord> {
    let decision_status = row.get::<_, String>(2)?;
    let decision = approval_decision_status_from_str(&decision_status).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(ApprovalDecisionRecord {
        approval_id: row.get(0)?,
        patch_id: row.get(1)?,
        decision,
        decided_by: row.get(3)?,
        reason_present: row.get::<_, i64>(4)? != 0,
        reason_summary: row.get(5)?,
    })
}

fn approval_decision_status_from_str(value: &str) -> CoreResult<ApprovalDecisionStatus> {
    match value {
        APPROVAL_DECISION_APPROVED => Ok(ApprovalDecisionStatus::Approved),
        APPROVAL_DECISION_DENIED => Ok(ApprovalDecisionStatus::Denied),
        other => Err(CoreError::InvalidApprovalDecisionStatus(other.to_string())),
    }
}

fn event_digest(envelope: &EventEnvelope) -> String {
    let mut hasher = Sha256::new();
    for field in [
        envelope.seq.to_string(),
        envelope.event_id.clone(),
        envelope.schema_version.to_string(),
        envelope.payload_schema_hash.clone(),
        envelope.actor_id.clone(),
        envelope.scope.clone(),
        envelope.workspace_id.clone(),
        envelope.idempotency_key.clone(),
        envelope.event_type.clone(),
        envelope.payload_summary.clone(),
    ] {
        hasher.update(field.len().to_string().as_bytes());
        hasher.update(b":");
        hasher.update(field.as_bytes());
        hasher.update(b";");
    }

    hex_digest(hasher.finalize().as_slice())
}

fn audit_hash(previous_hash: &str, event_digest: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(previous_hash.as_bytes());
    hasher.update(b":");
    hasher.update(event_digest.as_bytes());
    hex_digest(hasher.finalize().as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }

    output
}

fn insert_memory_note(
    tx: &Transaction<'_>,
    note_id: &str,
    workspace_id: &str,
    title: &str,
    content: &str,
    event_seq: u64,
) -> CoreResult<()> {
    let now = current_timestamp()?;
    tx.execute(
        "INSERT INTO memory_notes (note_id, workspace_id, title, content, created_at, updated_at, status, event_seq)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            note_id,
            workspace_id,
            title,
            content,
            checked_i64(now)?,
            checked_i64(now)?,
            "proposed",
            checked_i64(event_seq)?
        ],
    )?;
    Ok(())
}

fn update_memory_note_status(tx: &Transaction<'_>, note_id: &str, status: &str) -> CoreResult<()> {
    let now = current_timestamp()?;
    tx.execute(
        "UPDATE memory_notes SET status = ?2, updated_at = ?3 WHERE note_id = ?1",
        params![note_id, status, checked_i64(now)?],
    )?;
    Ok(())
}

fn memory_note_in_tx(tx: &Transaction<'_>, note_id: &str) -> CoreResult<Option<MemoryNoteRecord>> {
    tx.query_row(
        "SELECT note_id, workspace_id, title, content, created_at, updated_at, status
         FROM memory_notes WHERE note_id = ?1",
        params![note_id],
        |row| {
            Ok(MemoryNoteRecord {
                note_id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                created_at: checked_u64(row.get::<_, i64>(4)?)?,
                updated_at: checked_u64(row.get::<_, i64>(5)?)?,
                status: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(CoreError::from)
}

fn current_timestamp() -> CoreResult<u64> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| CoreError::WorkspaceMissing(format!("system time error: {e}")))?;
    Ok(duration.as_secs())
}

fn checked_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(CoreError::SequenceOutOfRange(value)),
        )
    })
}

fn checked_i64(value: u64) -> CoreResult<i64> {
    i64::try_from(value).map_err(|_| CoreError::SequenceOutOfRange(i64::MAX))
}

#[cfg(test)]
mod tests;
