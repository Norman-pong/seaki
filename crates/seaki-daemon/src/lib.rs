pub const DAEMON_ENTRYPOINT: &str = "local-daemon-ingress";

pub type SearchQueryInput = seaki_core::SearchQueryRequest;
pub type SearchResultDTO = seaki_core::SearchResultDTO;
pub type SessionSearchInput = seaki_core::SessionSearchRequest;
pub type SessionSearchResultDTO = seaki_core::SessionSearchResultDTO;
pub type SessionRedactInput = seaki_core::SessionRedactRequest;
pub type SessionRedactResult = seaki_core::SessionRedactResultDTO;

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
    /// 打开指定路径的持久化 ledger 并创建 Daemon。
    ///
    /// # Errors
    ///
    /// 当 ledger 打开失败时返回 `CoreError`。
    pub fn open(path: impl AsRef<std::path::Path>) -> seaki_core::CoreResult<Self> {
        Ok(Self::new(seaki_core::CoreLedger::open(path)?))
    }

    /// 在内存中创建 Daemon。
    ///
    /// # Errors
    ///
    /// 当内存 ledger 初始化失败时返回 `CoreError`。
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
    /// 初始化新的工作区。
    ///
    /// # Errors
    ///
    /// 当 ledger 写入失败时返回 `L::Error`。
    pub fn workspace_init(
        &mut self,
        input: WorkspaceInitInput,
    ) -> Result<WorkspaceInitResult, L::Error> {
        self.ledger.workspace_init(input)
    }

    /// 接收外部请求并将其作为 inert 事件追加到 ledger。
    ///
    /// # Errors
    ///
    /// 当事件追加失败时返回 `L::Error`。
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

    /// 从指定序列号开始重放 ledger 中的事件。
    ///
    /// # Errors
    ///
    /// 当读取或转换事件失败时返回 `L::Error`。
    pub fn replay(&self, from_seq: EventSeq) -> Result<Vec<LedgerEvent>, L::Error> {
        self.ledger.replay(from_seq)
    }

    /// 执行搜索查询。
    ///
    /// # Errors
    ///
    /// 当查询执行失败时返回 `L::Error`。
    pub fn search_query(&self, input: SearchQueryInput) -> Result<Vec<SearchResultDTO>, L::Error> {
        self.ledger.search_query(input)
    }

    pub fn pipe_list(
        &self,
        filter: Option<&seaki_pipe::SideEffectFilter>,
    ) -> Vec<seaki_pipe::PipeCommandSummary> {
        self.ledger.pipe_list(filter)
    }

    /// 查看指定管道命令的详细配置。
    ///
    /// # Errors
    ///
    /// 当命令不存在时返回 `CommandNotFound`。
    pub fn pipe_inspect(
        &self,
        command_id: &str,
    ) -> Result<seaki_pipe::PipeCommandManifest, seaki_pipe::CommandNotFound> {
        self.ledger.pipe_inspect(command_id)
    }

    /// 对管道 AST 进行 dry run 并返回结果。
    ///
    /// # Errors
    ///
    /// 当 dry run 执行失败时返回 `L::Error`。
    pub fn pipe_dry_run(
        &self,
        ast: &seaki_pipe::PipelineAst,
        initial_input: serde_json::Value,
    ) -> Result<seaki_pipe::DryRunResult, L::Error> {
        self.ledger.pipe_dry_run(ast, initial_input)
    }

    /// 提议创建或更新一条记忆。
    ///
    /// # Errors
    ///
    /// 当写入 ledger 失败时返回 `L::Error`。
    pub fn memory_propose(
        &mut self,
        input: MemoryProposeInput,
    ) -> Result<MemoryProposeResult, L::Error> {
        self.ledger.memory_propose(input)
    }

    /// 提交记忆变更。
    ///
    /// # Errors
    ///
    /// 当写入 ledger 失败时返回 `L::Error`。
    pub fn memory_commit(
        &mut self,
        input: MemoryCommitInput,
    ) -> Result<MemoryCommitResult, L::Error> {
        self.ledger.memory_commit(input)
    }

    /// 执行会话搜索查询。
    ///
    /// # Errors
    ///
    /// 当查询执行失败时返回 `L::Error`。
    pub fn session_search(
        &self,
        input: &SessionSearchInput,
    ) -> Result<Vec<SessionSearchResultDTO>, L::Error> {
        self.ledger.session_search(input)
    }

    /// 对会话进行脱敏并加入索引。
    ///
    /// # Errors
    ///
    /// 当写入 ledger 失败时返回 `L::Error`。
    pub fn session_redact(
        &mut self,
        input: SessionRedactInput,
    ) -> Result<SessionRedactResult, L::Error> {
        self.ledger.session_redact(input)
    }
}

pub trait CoreLedgerApi {
    type Error;

    /// 初始化新的工作区。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn workspace_init(
        &mut self,
        input: WorkspaceInitInput,
    ) -> Result<WorkspaceInitResult, Self::Error>;

    /// 追加一条 inert 事件到 ledger。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn append_inert_event(&mut self, event: InertEvent) -> Result<AppendedEvent, Self::Error>;

    /// 从指定序列号重放事件。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn replay(&self, from_seq: EventSeq) -> Result<Vec<LedgerEvent>, Self::Error>;

    /// 执行搜索查询。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn search_query(&self, input: SearchQueryInput) -> Result<Vec<SearchResultDTO>, Self::Error>;

    fn pipe_list(
        &self,
        filter: Option<&seaki_pipe::SideEffectFilter>,
    ) -> Vec<seaki_pipe::PipeCommandSummary>;

    /// 查看指定管道命令的详细配置。
    ///
    /// # Errors
    ///
    /// 当命令不存在时返回 `CommandNotFound`。
    fn pipe_inspect(
        &self,
        command_id: &str,
    ) -> Result<seaki_pipe::PipeCommandManifest, seaki_pipe::CommandNotFound>;

    /// 对管道 AST 进行 dry run。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn pipe_dry_run(
        &self,
        ast: &seaki_pipe::PipelineAst,
        initial_input: serde_json::Value,
    ) -> Result<seaki_pipe::DryRunResult, Self::Error>;

    /// 提议创建或更新一条记忆。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn memory_propose(
        &mut self,
        input: MemoryProposeInput,
    ) -> Result<MemoryProposeResult, Self::Error>;

    /// 提交记忆变更。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn memory_commit(
        &mut self,
        input: MemoryCommitInput,
    ) -> Result<MemoryCommitResult, Self::Error>;

    /// 执行会话搜索查询。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn session_search(
        &self,
        input: &SessionSearchInput,
    ) -> Result<Vec<SessionSearchResultDTO>, Self::Error>;

    /// 对会话进行脱敏并加入索引。
    ///
    /// # Errors
    ///
    /// 当操作失败时返回 `Self::Error`。
    fn session_redact(
        &mut self,
        input: SessionRedactInput,
    ) -> Result<SessionRedactResult, Self::Error>;
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
        filter: Option<&seaki_pipe::SideEffectFilter>,
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
        ast: &seaki_pipe::PipelineAst,
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

    fn session_search(
        &self,
        input: &SessionSearchInput,
    ) -> Result<Vec<SessionSearchResultDTO>, Self::Error> {
        seaki_core::CoreLedger::session_search(self, input)
    }

    fn session_redact(
        &mut self,
        input: SessionRedactInput,
    ) -> Result<SessionRedactResult, Self::Error> {
        seaki_core::CoreLedger::session_redact(self, input)
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
