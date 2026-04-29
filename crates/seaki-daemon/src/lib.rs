pub const DAEMON_ENTRYPOINT: &str = "local-daemon-ingress";

pub type SearchQueryInput = seaki_core::SearchQueryRequest;
pub type SearchResultDTO = seaki_core::SearchResultDTO;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonIngressContract {
    pub accepts_inert_events: bool,
    pub exposes_frontend_api: bool,
}

impl DaemonIngressContract {
    #[must_use]
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
mod tests;
