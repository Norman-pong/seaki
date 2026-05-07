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
    assert!(matches!(result, Err(seaki_pipe::CommandNotFound(ref id)) if id == "unknown.command"));
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
                args: serde_json::json!({}),
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s2".to_string(),
                command_id: "citation.resolve".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                args: serde_json::json!({}),
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s3".to_string(),
                command_id: "wiki.patch.propose".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                args: serde_json::json!({}),
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
        ],
    };
    let result = daemon
        .pipe_dry_run(&ast, serde_json::json!({"keyword": "rust"}))
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
