pub const DAEMON_ENTRYPOINT: &str = "local-daemon-ingress";

pub type SearchQueryInput = seaki_core::SearchQueryRequest;
pub type SearchResultDTO = seaki_core::SearchResultDTO;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonIngressContract {
    pub accepts_inert_events: bool,
    pub exposes_frontend_api: bool,
}

impl DaemonIngressContract {
    pub const fn m0() -> Self {
        Self {
            accepts_inert_events: true,
            exposes_frontend_api: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Daemon<L> {
    ledger: L,
}

impl Daemon<seaki_core::CoreLedger> {
    pub fn open(path: impl AsRef<std::path::Path>) -> seaki_core::CoreResult<Self> {
        Ok(Self::new(seaki_core::CoreLedger::open(path)?))
    }

    pub fn open_in_memory() -> seaki_core::CoreResult<Self> {
        Ok(Self::new(seaki_core::CoreLedger::open_in_memory()?))
    }
}

impl<L> Daemon<L> {
    pub fn new(ledger: L) -> Self {
        Self { ledger }
    }

    pub fn into_inner(self) -> L {
        self.ledger
    }
}

impl<L> Daemon<L>
where
    L: CoreLedgerApi,
{
    pub fn workspace_init(
        &mut self,
        input: WorkspaceInitInput,
    ) -> Result<WorkspaceInitResult, L::Error> {
        self.ledger.workspace_init(input)
    }

    pub fn ingress(&mut self, request: IngressRequest) -> Result<AppendedEvent, L::Error> {
        let event = InertEvent {
            event_id: request.event_id,
            origin: request.origin,
            actor: request.actor,
            workspace_id: request.workspace_id,
            scope: request.scope,
            schema_version: request.schema_version,
            payload_schema_hash: request.payload_schema_hash,
            idempotency_key: request.idempotency_key,
            event_type: request.event_type,
            payload: request.payload,
        };

        self.ledger.append_inert_event(event)
    }

    pub fn replay(&self, from_seq: EventSeq) -> Result<Vec<LedgerEvent>, L::Error> {
        self.ledger.replay(from_seq)
    }

    pub fn search_query(&self, input: SearchQueryInput) -> Result<Vec<SearchResultDTO>, L::Error> {
        self.ledger.search_query(input)
    }

    pub fn pipe_list(
        &self,
        filter: Option<seaki_pipe::SideEffectFilter>,
    ) -> Vec<seaki_pipe::PipeCommandSummary> {
        self.ledger.pipe_list(filter)
    }

    pub fn pipe_inspect(
        &self,
        command_id: &str,
    ) -> Result<seaki_pipe::PipeCommandManifest, seaki_pipe::CommandNotFound> {
        self.ledger.pipe_inspect(command_id)
    }

    pub fn pipe_dry_run(
        &self,
        ast: seaki_pipe::PipelineAst,
        initial_input: serde_json::Value,
    ) -> Result<seaki_pipe::DryRunResult, L::Error> {
        self.ledger.pipe_dry_run(ast, initial_input)
    }

    pub fn memory_propose(
        &mut self,
        input: MemoryProposeInput,
    ) -> Result<MemoryProposeResult, L::Error> {
        self.ledger.memory_propose(input)
    }

    pub fn memory_commit(
        &mut self,
        input: MemoryCommitInput,
    ) -> Result<MemoryCommitResult, L::Error> {
        self.ledger.memory_commit(input)
    }
}

pub trait CoreLedgerApi {
    type Error;

    fn workspace_init(
        &mut self,
        input: WorkspaceInitInput,
    ) -> Result<WorkspaceInitResult, Self::Error>;

    fn append_inert_event(&mut self, event: InertEvent) -> Result<AppendedEvent, Self::Error>;

    fn replay(&self, from_seq: EventSeq) -> Result<Vec<LedgerEvent>, Self::Error>;

    fn search_query(&self, input: SearchQueryInput) -> Result<Vec<SearchResultDTO>, Self::Error>;

    fn pipe_list(
        &self,
        filter: Option<seaki_pipe::SideEffectFilter>,
    ) -> Vec<seaki_pipe::PipeCommandSummary>;

    fn pipe_inspect(
        &self,
        command_id: &str,
    ) -> Result<seaki_pipe::PipeCommandManifest, seaki_pipe::CommandNotFound>;

    fn pipe_dry_run(
        &self,
        ast: seaki_pipe::PipelineAst,
        initial_input: serde_json::Value,
    ) -> Result<seaki_pipe::DryRunResult, Self::Error>;

    fn memory_propose(
        &mut self,
        input: MemoryProposeInput,
    ) -> Result<MemoryProposeResult, Self::Error>;

    fn memory_commit(
        &mut self,
        input: MemoryCommitInput,
    ) -> Result<MemoryCommitResult, Self::Error>;
}

impl CoreLedgerApi for seaki_core::CoreLedger {
    type Error = seaki_core::CoreError;

    fn workspace_init(
        &mut self,
        input: WorkspaceInitInput,
    ) -> Result<WorkspaceInitResult, Self::Error> {
        let result = self.workspace_init(seaki_core::WorkspaceInitRequest::new(
            input.event_id,
            input.actor,
            input.workspace_id,
            input.idempotency_key,
            input.payload_summary,
        ))?;

        Ok(WorkspaceInitResult {
            workspace_id: result.workspace_id,
            revision: result.workspace_revision,
            audit_head: result.audit_head,
            index_status: result.index_status,
        })
    }

    fn append_inert_event(&mut self, event: InertEvent) -> Result<AppendedEvent, Self::Error> {
        let envelope = self.append_inert_event(seaki_core::InertEvent {
            event_id: event.event_id,
            schema_version: event.schema_version,
            payload_schema_hash: event.payload_schema_hash,
            actor_id: event.actor,
            scope: event.scope,
            workspace_id: event.workspace_id,
            idempotency_key: event.idempotency_key,
            event_type: event.event_type,
            payload_summary: event.payload.summary(),
        })?;
        let audit_head = self
            .audit_entries()?
            .into_iter()
            .find(|entry| entry.event_seq == envelope.seq)
            .map(|entry| entry.hash)
            .ok_or(seaki_core::CoreError::AuditMissingForEvent(envelope.seq))?;

        Ok(AppendedEvent {
            seq: EventSeq(envelope.seq),
            event_id: envelope.event_id,
            audit_head,
        })
    }

    fn replay(&self, from_seq: EventSeq) -> Result<Vec<LedgerEvent>, Self::Error> {
        self.replay_events_after(from_seq.0)?
            .into_iter()
            .map(LedgerEvent::try_from)
            .collect()
    }

    fn search_query(&self, input: SearchQueryInput) -> Result<Vec<SearchResultDTO>, Self::Error> {
        seaki_core::CoreLedger::search_query(self, input)
    }

    fn pipe_list(
        &self,
        filter: Option<seaki_pipe::SideEffectFilter>,
    ) -> Vec<seaki_pipe::PipeCommandSummary> {
        seaki_core::CoreLedger::pipe_list(self, filter)
    }

    fn pipe_inspect(
        &self,
        command_id: &str,
    ) -> Result<seaki_pipe::PipeCommandManifest, seaki_pipe::CommandNotFound> {
        seaki_core::CoreLedger::pipe_inspect(self, command_id)
    }

    fn pipe_dry_run(
        &self,
        ast: seaki_pipe::PipelineAst,
        initial_input: serde_json::Value,
    ) -> Result<seaki_pipe::DryRunResult, Self::Error> {
        seaki_core::CoreLedger::pipe_dry_run(self, ast, initial_input)
    }

    fn memory_propose(
        &mut self,
        input: MemoryProposeInput,
    ) -> Result<MemoryProposeResult, Self::Error> {
        let note_id = input.note_id.clone();
        let envelope = self.append_memory_propose(seaki_core::MemoryProposeRequest::new(
            input.event_id,
            input.actor,
            input.workspace_id,
            input.idempotency_key,
            input.note_id,
            input.title,
            input.content,
        ))?;

        Ok(MemoryProposeResult {
            note_id,
            seq: EventSeq(envelope.seq),
            event_id: envelope.event_id,
        })
    }

    fn memory_commit(
        &mut self,
        input: MemoryCommitInput,
    ) -> Result<MemoryCommitResult, Self::Error> {
        let note_id = input.note_id.clone();
        let envelope = self.append_memory_commit(seaki_core::MemoryCommitRequest::new(
            input.event_id,
            input.actor,
            input.workspace_id,
            input.idempotency_key,
            input.note_id,
            input.approval_id,
            input.committed_revision,
        ))?;

        Ok(MemoryCommitResult {
            note_id,
            seq: EventSeq(envelope.seq),
            event_id: envelope.event_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProposeInput {
    pub event_id: String,
    pub workspace_id: String,
    pub actor: String,
    pub idempotency_key: String,
    pub note_id: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProposeResult {
    pub note_id: String,
    pub seq: EventSeq,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCommitInput {
    pub event_id: String,
    pub workspace_id: String,
    pub actor: String,
    pub idempotency_key: String,
    pub note_id: String,
    pub approval_id: String,
    pub committed_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCommitResult {
    pub note_id: String,
    pub seq: EventSeq,
    pub event_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventSeq(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInitInput {
    pub event_id: String,
    pub workspace_id: String,
    pub actor: String,
    pub idempotency_key: String,
    pub payload_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInitResult {
    pub workspace_id: String,
    pub revision: u64,
    pub audit_head: String,
    pub index_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressRequest {
    pub event_id: String,
    pub origin: IngressOrigin,
    pub actor: String,
    pub workspace_id: String,
    pub scope: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub payload: InertPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngressOrigin {
    Frontend,
    Local,
    ChannelBridge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InertPayload {
    pub media_type: String,
    pub body: Vec<u8>,
}

impl InertPayload {
    fn summary(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InertEvent {
    pub event_id: String,
    pub origin: IngressOrigin,
    pub actor: String,
    pub workspace_id: String,
    pub scope: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub payload: InertPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendedEvent {
    pub seq: EventSeq,
    pub event_id: String,
    pub audit_head: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEvent {
    pub seq: EventSeq,
    pub event_id: String,
    pub schema_version: u32,
    pub payload_schema_hash: String,
    pub actor: String,
    pub scope: String,
    pub workspace_id: String,
    pub idempotency_key: String,
    pub event_type: String,
    pub payload_summary: String,
}

impl TryFrom<seaki_core::EventEnvelope> for LedgerEvent {
    type Error = seaki_core::CoreError;

    fn try_from(envelope: seaki_core::EventEnvelope) -> Result<Self, Self::Error> {
        Ok(Self {
            seq: EventSeq(envelope.seq),
            event_id: envelope.event_id,
            schema_version: envelope.schema_version,
            payload_schema_hash: envelope.payload_schema_hash,
            actor: envelope.actor_id,
            scope: envelope.scope,
            workspace_id: envelope.workspace_id,
            idempotency_key: envelope.idempotency_key,
            event_type: envelope.event_type,
            payload_summary: envelope.payload_summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seaki_index::{
        CandidateKind, IndexCandidateId, IndexGeneration, IndexScope, IndexedCitationRef,
        IndexedDocument, SourceRange, SourceRangeUnit, SourceStatus, Visibility,
    };

    #[test]
    fn daemon_m0_contract_is_inert_ingress() {
        let contract = DaemonIngressContract::m0();

        assert_eq!(DAEMON_ENTRYPOINT, "local-daemon-ingress");
        assert!(contract.accepts_inert_events);
        assert!(contract.exposes_frontend_api);
    }

    #[test]
    fn workspace_init_delegates_to_core_and_returns_core_state() {
        let mut daemon = Daemon::open_in_memory().expect("core ledger opens");

        let result = daemon
            .workspace_init(workspace_init_input())
            .expect("workspace init should be accepted by core");

        assert_eq!(result.workspace_id, "workspace-alpha");
        assert_eq!(result.revision, 1);
        assert_ne!(result.audit_head, seaki_core::GENESIS_AUDIT_HASH);
        assert_eq!(result.index_status, seaki_core::INDEX_STATUS_STALE);
    }

    #[test]
    fn ingress_writes_legal_inert_event_and_replay_returns_it() {
        let mut daemon = initialized_daemon();

        let appended = daemon
            .ingress(valid_request("event-2", "idem-2"))
            .expect("valid inert event should be accepted by core");

        assert_eq!(appended.seq, EventSeq(2));
        assert_eq!(appended.event_id, "event-2");
        assert_ne!(appended.audit_head, seaki_core::GENESIS_AUDIT_HASH);

        let replayed = daemon
            .replay(EventSeq(1))
            .expect("replay should be delegated to core");

        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].seq, EventSeq(2));
        assert_eq!(replayed[0].idempotency_key, "idem-2");
        assert_eq!(
            replayed[0].schema_version,
            seaki_core::CURRENT_EVENT_SCHEMA_VERSION
        );
        assert_eq!(replayed[0].scope, workspace_scope());
    }

    #[test]
    fn core_rejections_do_not_enter_replay() {
        let mut daemon = initialized_daemon();

        daemon
            .ingress(valid_request("event-2", "idem-2"))
            .expect("baseline event should be accepted");

        let mut invalid_schema = valid_request("event-3", "idem-3");
        invalid_schema.schema_version = seaki_core::CURRENT_EVENT_SCHEMA_VERSION + 1;
        assert!(matches!(
            daemon.ingress(invalid_schema),
            Err(seaki_core::CoreError::InvalidSchemaVersion { .. })
        ));

        let mut invalid_scope = valid_request("event-4", "idem-4");
        invalid_scope.scope = "workspace:other".to_string();
        assert!(matches!(
            daemon.ingress(invalid_scope),
            Err(seaki_core::CoreError::InvalidScope { .. })
        ));

        assert!(matches!(
            daemon.ingress(valid_request("event-5", "idem-2")),
            Err(seaki_core::CoreError::DuplicateIdempotencyKey(_))
        ));

        let replayed = daemon.replay(EventSeq(1)).expect("replay should work");

        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].event_id, "event-2");
        assert_eq!(replayed[0].idempotency_key, "idem-2");

        let ledger = daemon.into_inner();
        assert_eq!(ledger.event_count().expect("event count reads"), 2);
        assert_eq!(ledger.audit_count().expect("audit count reads"), 2);
    }

    #[test]
    fn search_query_delegates_to_core_authorization_path() {
        let daemon = search_daemon();

        let results = daemon
            .search_query(SearchQueryInput::new(
                "workspace-alpha",
                "account-alpha",
                "needle",
                10,
            ))
            .expect("search query delegates to core");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result_id, "doc-visible");
        assert_eq!(results[0].snippet.as_deref(), Some("allowed body"));
        assert_eq!(
            results[0].citation_refs[0].citation_id,
            "citation-doc-visible"
        );
        assert_eq!(
            results[0].index_status.state,
            seaki_core::INDEX_STATUS_FRESH
        );
    }

    fn initialized_daemon() -> Daemon<seaki_core::CoreLedger> {
        let mut daemon = Daemon::open_in_memory().expect("core ledger opens");
        daemon
            .workspace_init(workspace_init_input())
            .expect("workspace init should be accepted");
        daemon
    }

    fn search_daemon() -> Daemon<seaki_core::CoreLedger> {
        let mut ledger = seaki_core::CoreLedger::open_in_memory().expect("core ledger opens");
        ledger
            .workspace_init(seaki_core::WorkspaceInitRequest::new(
                "event-1",
                "user:local",
                "workspace-alpha",
                "idem-1",
                "initialize workspace",
            ))
            .expect("workspace init should be accepted");
        let scope = IndexScope::new("workspace-alpha", "account-alpha");
        ledger
            .replace_search_scope(
                IndexGeneration::fresh(1, scope.clone(), 1, 1),
                [
                    indexed_document(
                        "doc-visible",
                        &scope,
                        "needle",
                        "allowed body",
                        Visibility::Visible,
                    ),
                    indexed_document(
                        "doc-restricted",
                        &scope,
                        "needle",
                        "restricted body",
                        Visibility::Restricted,
                    ),
                ],
            )
            .expect("search scope seeds");
        Daemon::new(ledger)
    }

    fn indexed_document(
        id: &str,
        scope: &IndexScope,
        title: &str,
        body: &str,
        visibility: Visibility,
    ) -> IndexedDocument {
        IndexedDocument {
            candidate_id: IndexCandidateId::new(id),
            workspace_id: scope.workspace_id.clone(),
            account_id: scope.account_id.clone(),
            source_id: "source-1".to_string(),
            citation_ref: Some(IndexedCitationRef {
                citation_id: format!("citation-{id}"),
                source_id: "source-1".to_string(),
                range: SourceRange {
                    unit: SourceRangeUnit::Line,
                    start: 1,
                    end: 1,
                    label: Some("source-1:1".to_string()),
                },
                wiki_page_id: format!("page-{id}"),
                claim_id: format!("claim-{id}"),
                degraded_reason: None,
            }),
            kind: CandidateKind::Claim,
            title: title.to_string(),
            body: body.to_string(),
            visibility,
            source_status: SourceStatus::Active,
            source_revision: 1,
            wiki_revision: 1,
        }
    }

    fn workspace_init_input() -> WorkspaceInitInput {
        WorkspaceInitInput {
            event_id: "event-1".to_string(),
            workspace_id: "workspace-alpha".to_string(),
            actor: "user:local".to_string(),
            idempotency_key: "idem-1".to_string(),
            payload_summary: "initialize workspace".to_string(),
        }
    }

    fn valid_request(event_id: &str, idempotency_key: &str) -> IngressRequest {
        IngressRequest {
            event_id: event_id.to_string(),
            origin: IngressOrigin::Frontend,
            actor: "user:local".to_string(),
            workspace_id: "workspace-alpha".to_string(),
            scope: workspace_scope(),
            schema_version: seaki_core::CURRENT_EVENT_SCHEMA_VERSION,
            payload_schema_hash: "workspace.note.v1".to_string(),
            idempotency_key: idempotency_key.to_string(),
            event_type: "workspace.note".to_string(),
            payload: InertPayload {
                media_type: "application/json".to_string(),
                body: br#"{"kind":"note","text":"hello"}"#.to_vec(),
            },
        }
    }

    fn workspace_scope() -> String {
        seaki_core::workspace_scope("workspace-alpha")
    }

    #[test]
    fn pipe_list_delegates_to_core() {
        let daemon = initialized_daemon();
        let results = daemon.pipe_list(None);
        assert!(!results.is_empty());
        let ids: Vec<_> = results.iter().map(|r| r.command_id.as_str()).collect();
        assert!(ids.contains(&"wiki.search"));
        assert!(ids.contains(&"wiki.patch.propose"));
    }

    #[test]
    fn pipe_inspect_delegates_to_core() {
        let daemon = initialized_daemon();
        let manifest = daemon
            .pipe_inspect("wiki.search")
            .expect("wiki.search exists");
        assert_eq!(manifest.command_id, "wiki.search");
    }

    #[test]
    fn pipe_inspect_unknown_returns_command_not_found() {
        let daemon = initialized_daemon();
        let result = daemon.pipe_inspect("unknown.command");
        assert!(
            matches!(result, Err(seaki_pipe::CommandNotFound(ref id)) if id == "unknown.command")
        );
    }

    #[test]
    fn pipe_dry_run_delegates_to_core() {
        let daemon = initialized_daemon();
        let ast = seaki_pipe::PipelineAst {
            pipeline_id: "daemon-pipe".to_string(),
            steps: vec![
                seaki_pipe::PipelineStep {
                    step_id: "s1".to_string(),
                    command_id: "wiki.search".to_string(),
                    input_binding: seaki_pipe::InputBinding::Constant(
                        serde_json::json!({"keyword": "rust"}),
                    ),
                    failure_policy: seaki_pipe::FailurePolicy::FailFast,
                },
                seaki_pipe::PipelineStep {
                    step_id: "s2".to_string(),
                    command_id: "citation.resolve".to_string(),
                    input_binding: seaki_pipe::InputBinding::PreviousStep,
                    failure_policy: seaki_pipe::FailurePolicy::FailFast,
                },
                seaki_pipe::PipelineStep {
                    step_id: "s3".to_string(),
                    command_id: "wiki.patch.propose".to_string(),
                    input_binding: seaki_pipe::InputBinding::PreviousStep,
                    failure_policy: seaki_pipe::FailurePolicy::FailFast,
                },
            ],
        };
        let result = daemon
            .pipe_dry_run(ast, serde_json::json!({"keyword": "rust"}))
            .expect("dry run succeeds");
        assert!(
            result.proposal_artifact.is_some(),
            "expected proposal artifact"
        );
    }

    #[test]
    fn memory_propose_and_commit_lifecycle() {
        let mut daemon = initialized_daemon();

        // memory.propose
        let propose_result = daemon
            .memory_propose(MemoryProposeInput {
                event_id: "event-2".to_string(),
                workspace_id: "workspace-alpha".to_string(),
                actor: "user:local".to_string(),
                idempotency_key: "idem-2".to_string(),
                note_id: "note-1".to_string(),
                title: "rust tips".to_string(),
                content: "use borrow checker".to_string(),
            })
            .expect("memory propose succeeds");

        assert_eq!(propose_result.note_id, "note-1");
        assert_eq!(propose_result.seq, EventSeq(2));

        // note 状态应为 proposed
        let ledger = daemon.into_inner();
        let note = ledger
            .memory_note("note-1")
            .expect("note loads")
            .expect("note exists");
        assert_eq!(note.status, "proposed");
        assert_eq!(note.title, "rust tips");
    }
}
