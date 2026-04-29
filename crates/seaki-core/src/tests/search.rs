use super::*;

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
