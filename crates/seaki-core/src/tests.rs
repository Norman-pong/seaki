use super::*;
use seaki_index::{
    CandidateKind, IndexCandidateId, IndexGeneration, IndexScope, IndexedCitationRef,
    IndexedDocument, SourceRange, SourceRangeUnit, SourceStatus, Visibility,
};
use tempfile::NamedTempFile;

#[test]
fn core_names_its_authority_boundary() {
    assert_eq!(CORE_AUTHORITY, "policy-approved-core");
    assert!(owns_record_kind(CoreRecordKind::Transaction));
}

#[test]
fn workspace_init_returns_revision_audit_head_and_index_status() {
    let mut ledger = CoreLedger::open_in_memory().expect("ledger opens");
    let result = ledger
        .workspace_init(workspace_init_request("event-1", "idem-1"))
        .expect("workspace init succeeds");

    assert_eq!(result.workspace_id, "workspace-1");
    assert_eq!(result.workspace_revision, 1);
    assert_ne!(result.audit_head, GENESIS_AUDIT_HASH);
    assert_eq!(result.index_status, INDEX_STATUS_STALE);
    assert_eq!(
        ledger.audit_head("workspace-1").expect("audit head loads"),
        Some(result.audit_head)
    );
}

#[test]
fn file_backed_ledger_enables_wal_journal_mode() {
    let file = NamedTempFile::new().expect("temp sqlite file");
    let ledger = CoreLedger::open(file.path()).expect("ledger opens");

    assert_eq!(
        ledger
            .journal_mode()
            .expect("journal mode loads")
            .to_lowercase(),
        "wal"
    );
}

#[test]
fn search_query_returns_authorized_search_result_dtos_without_wal_write() {
    let mut ledger = initialized_ledger();
    seed_search_index(&mut ledger);
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    let results = ledger
        .search_query(SearchQueryRequest::new(
            "workspace-1",
            "account-1",
            "needle",
            10,
        ))
        .expect("search query succeeds");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].result_id, "doc-visible");
    assert_eq!(results[0].kind, "claim");
    assert_eq!(results[0].title, "needle");
    assert_eq!(results[0].snippet.as_deref(), Some("allowed cited body"));
    assert_eq!(results[0].index_status.state, INDEX_STATUS_FRESH);
    assert_eq!(
        results[0].index_status.last_good_revision.as_deref(),
        Some("1")
    );
    assert_eq!(
        results[0].citation_refs[0].citation_id,
        "citation-doc-visible"
    );
    assert_eq!(results[0].citation_refs[0].range.unit, "line");
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
}

#[test]
fn search_query_filters_uncited_candidate_without_wal_write() {
    let mut ledger = initialized_ledger();
    seed_search_index(&mut ledger);
    let scope = IndexScope::new("workspace-1", "account-1");
    ledger
        .replace_search_scope(
            IndexGeneration::fresh(2, scope.clone(), 1, 2),
            [uncited_document(
                "doc-uncited",
                &scope,
                "needle",
                "uncited body",
            )],
        )
        .expect("search scope replaces");
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    let results = ledger
        .search_query(SearchQueryRequest::new(
            "workspace-1",
            "account-1",
            "needle",
            10,
        ))
        .expect("search query succeeds");

    assert!(results.is_empty());
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
}

#[test]
fn search_query_missing_workspace_does_not_write() {
    let ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    assert!(matches!(
        ledger.search_query(SearchQueryRequest::new(
            "workspace-missing",
            "account-1",
            "needle",
            10,
        )),
        Err(CoreError::WorkspaceMissing(_))
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
}

#[test]
fn replay_events_after_returns_events_in_seq_order() {
    let mut ledger = initialized_ledger();

    let first = ledger
        .append_inert_event(test_event("event-2", "idem-2", "workspace.note"))
        .expect("first event appends");
    let second = ledger
        .append_inert_event(test_event("event-3", "idem-3", "workspace.note"))
        .expect("second event appends");
    let replayed = ledger.replay_events_after(1).expect("events replay");

    assert_eq!(replayed, vec![first, second]);
    assert!(replayed[0].seq < replayed[1].seq);
}

#[test]
fn invalid_schema_scope_and_duplicate_idempotency_do_not_write_events_or_audit() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    let mut invalid_schema = test_event("event-2", "idem-2", "workspace.note");
    invalid_schema.schema_version = CURRENT_EVENT_SCHEMA_VERSION + 1;
    assert!(matches!(
        ledger.append_inert_event(invalid_schema),
        Err(CoreError::InvalidSchemaVersion { .. })
    ));

    let mut invalid_payload_schema_hash = test_event("event-3", "idem-3", "workspace.note");
    invalid_payload_schema_hash.payload_schema_hash = "other.schema.v1".to_string();
    assert!(matches!(
        ledger.append_inert_event(invalid_payload_schema_hash),
        Err(CoreError::InvalidPayloadSchemaHash { .. })
    ));

    let mut invalid_scope = test_event("event-3", "idem-3", "workspace.note");
    invalid_scope.scope = "workspace:other".to_string();
    assert!(matches!(
        ledger.append_inert_event(invalid_scope),
        Err(CoreError::InvalidScope { .. })
    ));

    assert!(matches!(
        ledger.append_inert_event(test_event("event-4", "idem-1", "workspace.note")),
        Err(CoreError::DuplicateIdempotencyKey(_))
    ));

    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
}

#[test]
fn audit_hash_chain_appends_and_uses_sanitized_summary() {
    let mut ledger = initialized_ledger();
    ledger
        .append_inert_event(test_event("event-2", "idem-2", "workspace.token_observed"))
        .expect("event appends");

    let audit_entries = ledger.audit_entries().expect("audit entries load");
    assert_eq!(audit_entries.len(), 2);
    assert_eq!(audit_entries[0].previous_hash, GENESIS_AUDIT_HASH);
    assert_eq!(audit_entries[1].previous_hash, audit_entries[0].hash);
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));

    let replayed = ledger.replay_events_after(1).expect("events replay");
    assert_eq!(replayed.len(), 1);
    assert!(!replayed[0].payload_summary.contains("abc123"));
    assert!(!replayed[0].payload_summary.contains("super-secret"));
    assert!(!replayed[0]
        .payload_summary
        .to_lowercase()
        .contains("bearer"));
    assert!(replayed[0].payload_summary.contains("[REDACTED]"));
}

#[test]
fn wiki_patch_commit_appends_wal_audit_and_marks_index_stale() {
    let mut ledger = initialized_ledger();
    ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        ))
        .expect("approval decision appends");

    let envelope = ledger
        .append_wiki_patch_commit(wiki_patch_commit_request(
            "event-3",
            "idem-3",
            3,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        ))
        .expect("wiki patch commit appends");

    assert_eq!(envelope.event_type, WIKI_PATCH_COMMIT_EVENT_TYPE);
    assert_eq!(
        envelope.payload_schema_hash,
        expected_payload_schema_hash(WIKI_PATCH_COMMIT_EVENT_TYPE)
    );
    assert_eq!(
        ledger
            .workspace_revision("workspace-1")
            .expect("revision loads"),
        Some(3)
    );
    assert_eq!(
        ledger.index_status("workspace-1").expect("index status"),
        Some(INDEX_STATUS_STALE.to_string())
    );
    assert_eq!(ledger.audit_count().expect("audit count"), 3);
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));
}

#[test]
fn wiki_patch_commit_rejects_revision_mismatch_before_wal_write() {
    let mut ledger = initialized_ledger();
    ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        ))
        .expect("approval decision appends");
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    assert!(matches!(
        ledger.append_wiki_patch_commit(wiki_patch_commit_request(
            "event-3",
            "idem-3",
            99,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        )),
        Err(CoreError::WorkspaceRevisionMismatch {
            expected: 3,
            found: 99
        })
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
    assert_eq!(
        ledger
            .workspace_revision("workspace-1")
            .expect("revision loads"),
        Some(2)
    );
}

#[test]
fn wiki_patch_commit_rejects_missing_or_denied_approval_decision_before_wal_write() {
    let mut missing_ledger = initialized_ledger();
    let missing_events = missing_ledger.event_count().expect("event count");
    let missing_audit = missing_ledger.audit_count().expect("audit count");

    assert!(matches!(
        missing_ledger.append_wiki_patch_commit(wiki_patch_commit_request(
            "event-2",
            "idem-2",
            2,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        )),
        Err(CoreError::ApprovalDecisionRequired { .. })
    ));
    assert_eq!(
        missing_ledger.event_count().expect("event count"),
        missing_events
    );
    assert_eq!(
        missing_ledger.audit_count().expect("audit count"),
        missing_audit
    );

    let mut denied_ledger = initialized_ledger();
    denied_ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Denied,
            Some("citation does not support claim".to_string()),
        ))
        .expect("denied approval records");
    let denied_events = denied_ledger.event_count().expect("event count");
    let denied_audit = denied_ledger.audit_count().expect("audit count");

    assert!(matches!(
        denied_ledger.append_wiki_patch_commit(wiki_patch_commit_request(
            "event-3",
            "idem-3",
            3,
            "txn-1",
            "patch-1",
            "approval-1",
            "rollback-1",
        )),
        Err(CoreError::ApprovalDecisionNotApproved { .. })
    ));
    assert_eq!(
        denied_ledger.event_count().expect("event count"),
        denied_events
    );
    assert_eq!(
        denied_ledger.audit_count().expect("audit count"),
        denied_audit
    );
}

#[test]
fn approved_decision_appends_wal_audit_and_record() {
    let mut ledger = initialized_ledger();
    let record = ApprovalDecisionRecord::new(
        "approval-1",
        "patch-1",
        ApprovalDecisionStatus::Approved,
        "user-1",
        None,
    );

    let envelope = ledger
        .append_approval_decision(ApprovalDecisionRequest::new(
            "event-2",
            "actor-1",
            "workspace-1",
            "idem-2",
            record.clone(),
        ))
        .expect("approval decision appends");

    assert_eq!(envelope.event_type, APPROVAL_DECISION_EVENT_TYPE);
    assert_eq!(
        envelope.payload_schema_hash,
        expected_payload_schema_hash(APPROVAL_DECISION_EVENT_TYPE)
    );
    assert!(envelope.payload_summary.contains("decision=approved"));
    assert!(envelope.payload_summary.contains("reason_present=false"));
    assert_eq!(
        ledger
            .approval_decision("approval-1")
            .expect("approval decision loads"),
        Some(record)
    );
    assert_eq!(
        ledger
            .workspace_revision("workspace-1")
            .expect("revision loads"),
        Some(2)
    );
    assert_eq!(ledger.audit_count().expect("audit count"), 2);
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));
}

#[test]
fn denied_decision_appends_wal_audit_with_sanitized_reason_summary() {
    let mut ledger = initialized_ledger();
    let reason = format!(
        "Bearer abc123 token=super-secret {}",
        "rejection explanation ".repeat(12)
    );

    let envelope = ledger
        .append_approval_decision(approval_decision_request(
            "event-2",
            "idem-2",
            "approval-1",
            "patch-1",
            ApprovalDecisionStatus::Denied,
            Some(reason),
        ))
        .expect("approval denial appends");

    assert_eq!(envelope.event_type, APPROVAL_DECISION_EVENT_TYPE);
    assert!(envelope.payload_summary.contains("decision=denied"));
    assert!(envelope.payload_summary.contains("reason_present=true"));
    assert!(!envelope.payload_summary.contains("abc123"));
    assert!(!envelope.payload_summary.contains("super-secret"));
    assert!(!envelope.payload_summary.to_lowercase().contains("bearer"));

    let stored = ledger
        .approval_decision("approval-1")
        .expect("approval decision loads")
        .expect("approval decision exists");
    let reason_summary = stored.reason_summary.expect("reason summary stored");
    assert!(stored.reason_present);
    assert!(reason_summary.chars().count() <= 96);
    assert!(!reason_summary.contains("abc123"));
    assert!(!reason_summary.contains("super-secret"));
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));
}

#[test]
fn approval_decision_validation_failures_do_not_write_events_or_audit() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    let mut invalid_schema = approval_decision_request(
        "event-2",
        "idem-2",
        "approval-1",
        "patch-1",
        ApprovalDecisionStatus::Approved,
        None,
    );
    invalid_schema.schema_version = CURRENT_EVENT_SCHEMA_VERSION + 1;
    assert!(matches!(
        ledger.append_approval_decision(invalid_schema),
        Err(CoreError::InvalidSchemaVersion { .. })
    ));

    let mut invalid_scope = approval_decision_request(
        "event-3",
        "idem-3",
        "approval-2",
        "patch-1",
        ApprovalDecisionStatus::Approved,
        None,
    );
    invalid_scope.scope = "workspace:other".to_string();
    assert!(matches!(
        ledger.append_approval_decision(invalid_scope),
        Err(CoreError::InvalidScope { .. })
    ));

    assert!(matches!(
        ledger.append_approval_decision(approval_decision_request(
            "event-4",
            "idem-1",
            "approval-3",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        )),
        Err(CoreError::DuplicateIdempotencyKey(_))
    ));

    assert!(matches!(
        ledger.append_approval_decision(approval_decision_request(
            "event-1",
            "idem-4",
            "approval-4",
            "patch-1",
            ApprovalDecisionStatus::Approved,
            None,
        )),
        Err(CoreError::DuplicateEventId(_))
    ));

    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
    assert!(ledger
        .approval_decision("approval-1")
        .expect("approval decision loads")
        .is_none());
}

#[test]
fn approval_decision_workspace_missing_does_not_write() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    assert!(matches!(
        ledger.append_approval_decision(ApprovalDecisionRequest::new(
            "event-2",
            "actor-1",
            "workspace-missing",
            "idem-2",
            ApprovalDecisionRecord::new(
                "approval-1",
                "patch-1",
                ApprovalDecisionStatus::Denied,
                "user-1",
                Some("not enough citation evidence".to_string()),
            ),
        )),
        Err(CoreError::WorkspaceMissing(_))
    ));

    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
    assert!(ledger
        .approval_decision("approval-1")
        .expect("approval decision loads")
        .is_none());
}

#[test]
fn audit_hash_chain_detects_event_tampering() {
    let mut ledger = initialized_ledger();
    ledger
        .append_inert_event(test_event("event-2", "idem-2", "workspace.note"))
        .expect("event appends");
    assert!(ledger.verify_audit_chain().expect("audit chain verifies"));

    ledger
        .conn
        .execute(
            "UPDATE events SET payload_summary = 'tampered after audit' WHERE seq = ?1",
            params![2_i64],
        )
        .expect("test tamper update succeeds");

    assert!(!ledger.verify_audit_chain().expect("audit chain loads"));
}

#[test]
fn duplicate_workspace_init_is_rejected_before_write() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let initial_audit = ledger.audit_count().expect("audit count");

    assert!(matches!(
        ledger.workspace_init(workspace_init_request("event-2", "idem-2")),
        Err(CoreError::WorkspaceAlreadyExists(_))
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
    assert_eq!(ledger.audit_count().expect("audit count"), initial_audit);
}

#[test]
fn empty_idempotency_key_is_rejected_before_write() {
    let mut ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");
    let mut event = test_event("event-2", "", "workspace.note");
    event.idempotency_key = "  ".to_string();

    assert!(matches!(
        ledger.append_inert_event(event),
        Err(CoreError::EmptyIdempotencyKey)
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
}

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

#[test]
fn citation_resolve_returns_source_range_for_visible_citation() {
    let mut ledger = initialized_ledger();
    seed_search_index(&mut ledger);

    let result = ledger
        .citation_resolve(CitationResolveRequest::new(
            "workspace-1",
            "account-1",
            "citation-doc-visible",
        ))
        .expect("citation resolve succeeds");

    assert_eq!(result.citation_id, "citation-doc-visible");
    assert_eq!(result.source_id, "source-1");
    assert_eq!(result.preview_target, "source_range");
    assert!(result.degraded_reason.is_none());
    assert!(result.source_card.is_some());
}

#[test]
fn citation_resolve_returns_no_access_for_missing_citation() {
    let mut ledger = initialized_ledger();
    seed_search_index(&mut ledger);

    let result = ledger
        .citation_resolve(CitationResolveRequest::new(
            "workspace-1",
            "account-1",
            "citation-missing",
        ))
        .expect("citation resolve succeeds");

    assert_eq!(result.preview_target, "none");
    assert!(result.degraded_reason.is_some());
    assert!(result.source_card.is_none());
}

#[test]
fn compose_answer_includes_only_visible_citations() {
    let mut ledger = initialized_ledger();
    seed_search_index(&mut ledger);

    let answer = ledger
        .compose_answer(AnswerComposerRequest::new(
            "workspace-1",
            "account-1",
            "needle",
            vec!["doc-visible".to_string()],
        ))
        .expect("compose answer succeeds");

    assert_eq!(answer.status, "composed");
    assert!(!answer.text.is_empty());
    assert_eq!(answer.citation_refs.len(), 1);
    assert_eq!(answer.citation_refs[0].citation_id, "citation-doc-visible");
}

#[test]
fn compose_answer_returns_no_access_when_no_visible_candidates() {
    let mut ledger = initialized_ledger();
    seed_search_index(&mut ledger);

    let answer = ledger
        .compose_answer(AnswerComposerRequest::new(
            "workspace-1",
            "account-1",
            "nonexistent",
            vec![],
        ))
        .expect("compose answer succeeds");

    assert_eq!(answer.status, "no_access");
    assert!(answer.text.is_empty());
    assert!(answer.citation_refs.is_empty());
}

#[test]
fn m0_happy_path_source_to_citation_backed_answer() {
    let mut ledger = initialized_ledger();
    let scope = IndexScope::new("workspace-1", "account-1");

    ledger
        .replace_search_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [indexed_document(
                "doc-decision",
                &scope,
                "source-1",
                "M0 decision",
                "workspace source boundary restricts file selection to authorized paths",
                Visibility::Visible,
                SourceStatus::Active,
            )],
        )
        .expect("seed search scope");

    let results = ledger
        .search_query(SearchQueryRequest::new(
            "workspace-1",
            "account-1",
            "boundary",
            10,
        ))
        .expect("search query succeeds");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].citation_refs.len(), 1);

    let citation_id = &results[0].citation_refs[0].citation_id;
    let resolved = ledger
        .citation_resolve(CitationResolveRequest::new(
            "workspace-1",
            "account-1",
            citation_id,
        ))
        .expect("citation resolve succeeds");
    assert_eq!(resolved.preview_target, "source_range");
    assert!(resolved.source_card.is_some());

    let answer = ledger
        .compose_answer(AnswerComposerRequest::new(
            "workspace-1",
            "account-1",
            "boundary",
            vec!["doc-decision".to_string()],
        ))
        .expect("compose answer succeeds");
    assert_eq!(answer.status, "composed");
    assert!(!answer.text.is_empty());
    assert_eq!(answer.citation_refs.len(), 1);
    assert_eq!(answer.citation_refs[0].citation_id, *citation_id);
}

#[test]
fn m0_reject_path_citation_resolve_returns_no_access_for_tombstoned_source() {
    let mut ledger = initialized_ledger();
    let scope = IndexScope::new("workspace-1", "account-1");

    ledger
        .replace_search_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [indexed_document(
                "doc-tombstoned",
                &scope,
                "source-tombstoned",
                "hidden",
                "content",
                Visibility::Tombstoned,
                SourceStatus::Tombstoned,
            )],
        )
        .expect("seed search scope");

    let resolved = ledger
        .citation_resolve(CitationResolveRequest::new(
            "workspace-1",
            "account-1",
            "citation-doc-tombstoned",
        ))
        .expect("citation resolve succeeds");
    assert_eq!(resolved.preview_target, "none");
    assert!(resolved.degraded_reason.is_some());
    assert!(resolved.source_card.is_none());

    let answer = ledger
        .compose_answer(AnswerComposerRequest::new(
            "workspace-1",
            "account-1",
            "hidden",
            vec!["doc-tombstoned".to_string()],
        ))
        .expect("compose answer succeeds");
    assert_eq!(answer.status, "no_access");
    assert!(answer.citation_refs.is_empty());
}

#[test]
fn m0_reject_path_search_excludes_restricted_candidates_from_authorization() {
    let mut ledger = initialized_ledger();
    let scope = IndexScope::new("workspace-1", "account-1");

    ledger
        .replace_search_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [
                indexed_document(
                    "doc-visible",
                    &scope,
                    "source-1",
                    "visible",
                    "allowed content",
                    Visibility::Visible,
                    SourceStatus::Active,
                ),
                indexed_document(
                    "doc-restricted",
                    &scope,
                    "source-1",
                    "restricted",
                    "restricted content",
                    Visibility::Restricted,
                    SourceStatus::Active,
                ),
            ],
        )
        .expect("seed search scope");

    let results = ledger
        .search_query(SearchQueryRequest::new(
            "workspace-1",
            "account-1",
            "visible",
            10,
        ))
        .expect("search query succeeds");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].result_id, "doc-visible");
}
