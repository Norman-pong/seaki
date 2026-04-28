pub const DAEMON_ENTRYPOINT: &str = "local-daemon-ingress";

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
}

pub trait CoreLedgerApi {
    type Error;

    fn workspace_init(
        &mut self,
        input: WorkspaceInitInput,
    ) -> Result<WorkspaceInitResult, Self::Error>;

    fn append_inert_event(&mut self, event: InertEvent) -> Result<AppendedEvent, Self::Error>;

    fn replay(&self, from_seq: EventSeq) -> Result<Vec<LedgerEvent>, Self::Error>;
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

    fn initialized_daemon() -> Daemon<seaki_core::CoreLedger> {
        let mut daemon = Daemon::open_in_memory().expect("core ledger opens");
        daemon
            .workspace_init(workspace_init_input())
            .expect("workspace init should be accepted");
        daemon
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
}
