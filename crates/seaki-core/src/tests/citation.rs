use super::*;

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
        .compose_answer(&AnswerComposerRequest::new(
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
        .compose_answer(&AnswerComposerRequest::new(
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
        .compose_answer(&AnswerComposerRequest::new(
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
        .compose_answer(&AnswerComposerRequest::new(
            "workspace-1",
            "account-1",
            "hidden",
            vec!["doc-tombstoned".to_string()],
        ))
        .expect("compose answer succeeds");
    assert_eq!(answer.status, "no_access");
    assert!(answer.citation_refs.is_empty());
}
