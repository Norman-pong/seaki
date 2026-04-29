use super::*;

#[test]
fn index_m0_contract_starts_from_candidate_ids() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 7, 11),
            [document(
                "doc-1",
                &scope,
                "rust search",
                "bm25 candidate search",
            )],
        )
        .unwrap();

    let search = index.search_candidates(&SearchQuery::new(
        "workspace-a",
        "account-a",
        "candidate",
        10,
    ));

    assert_eq!(search.disclosure, SEARCH_RESULT_DISCLOSURE);
    assert_eq!(search.candidate_ids, vec![IndexCandidateId::new("doc-1")]);
    assert_eq!(search.status, IndexStatus::Fresh);
}

#[test]
fn index_generation_records_freshness_scope_and_revisions() {
    let scope = IndexScope::new("workspace-a", "account-a");
    let generation = IndexGeneration::fresh_with_schema(42, scope, "schema.test", 3, 5);

    assert!(generation.is_fresh());
    assert!(!generation.is_stale());
    assert_eq!(generation.schema_version, "schema.test");
    assert_eq!(generation.workspace_id, "workspace-a");
    assert_eq!(generation.account_id, "account-a");
    assert_eq!(generation.source_revision, 3);
    assert_eq!(generation.wiki_revision, 5);
}

#[test]
fn bm25_ranks_stronger_candidate_first() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [
                document("doc-low", &scope, "general", "candidate search index"),
                document(
                    "doc-high",
                    &scope,
                    "bm25",
                    "bm25 bm25 candidate search ranking",
                ),
            ],
        )
        .unwrap();

    let search = index.search_candidates(&SearchQuery::new(
        "workspace-a",
        "account-a",
        "bm25 candidate",
        10,
    ));

    assert_eq!(
        search.candidate_ids,
        vec![
            IndexCandidateId::new("doc-high"),
            IndexCandidateId::new("doc-low")
        ]
    );
}

#[test]
fn search_is_workspace_and_account_scoped() {
    let mut index = Bm25CandidateIndex::new();
    let scope_a = IndexScope::new("workspace-a", "account-a");
    let scope_b = IndexScope::new("workspace-b", "account-a");
    let account_b = IndexScope::new("workspace-a", "account-b");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope_a.clone(), 1, 1),
            [document("doc-a", &scope_a, "needle", "visible only in a")],
        )
        .unwrap();
    index
        .replace_scope(
            IndexGeneration::fresh(2, scope_b.clone(), 1, 1),
            [document("doc-b", &scope_b, "needle", "wrong workspace")],
        )
        .unwrap();
    index
        .replace_scope(
            IndexGeneration::fresh(3, account_b.clone(), 1, 1),
            [document("doc-c", &account_b, "needle", "wrong account")],
        )
        .unwrap();

    let search =
        index.search_candidates(&SearchQuery::new("workspace-a", "account-a", "needle", 10));

    assert_eq!(search.candidate_ids, vec![IndexCandidateId::new("doc-a")]);
}

#[test]
fn same_candidate_id_is_isolated_by_scope() {
    let mut index = Bm25CandidateIndex::new();
    let scope_a = IndexScope::new("workspace-a", "account-a");
    let scope_b = IndexScope::new("workspace-b", "account-a");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope_a.clone(), 1, 1),
            [document("shared-doc", &scope_a, "alpha", "workspace a")],
        )
        .unwrap();
    index
        .replace_scope(
            IndexGeneration::fresh(2, scope_b.clone(), 1, 1),
            [document("shared-doc", &scope_b, "beta", "workspace b")],
        )
        .unwrap();

    let search_a =
        index.search_candidates(&SearchQuery::new("workspace-a", "account-a", "alpha", 10));
    let search_b =
        index.search_candidates(&SearchQuery::new("workspace-b", "account-a", "beta", 10));

    assert_eq!(
        search_a.candidate_ids,
        vec![IndexCandidateId::new("shared-doc")]
    );
    assert_eq!(
        search_b.candidate_ids,
        vec![IndexCandidateId::new("shared-doc")]
    );
}

#[test]
fn restricted_and_tombstoned_candidates_do_not_become_authorized_results() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    let mut restricted = document("doc-restricted", &scope, "needle", "restricted");
    restricted.visibility = Visibility::Restricted;
    let mut tombstoned = document("doc-tombstoned", &scope, "needle", "tombstoned");
    tombstoned.visibility = Visibility::Tombstoned;
    tombstoned.source_status = SourceStatus::Tombstoned;
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [
                document("doc-visible", &scope, "needle", "allowed"),
                restricted,
                tombstoned,
            ],
        )
        .unwrap();

    let query = SearchQuery::new("workspace-a", "account-a", "needle", 10);
    let candidate_ids = vec![
        IndexCandidateId::new("doc-visible"),
        IndexCandidateId::new("doc-restricted"),
        IndexCandidateId::new("doc-tombstoned"),
    ];
    let results = index.authorize_candidates(&query, &candidate_ids);

    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].candidate_id,
        IndexCandidateId::new("doc-visible")
    );
    assert_eq!(results[0].title, "needle");
    assert_eq!(results[0].snippet.as_deref(), Some("allowed"));
    assert_eq!(
        results[0].citation_refs[0].citation_id,
        "citation-doc-visible"
    );
}

#[test]
fn initially_restricted_and_tombstoned_candidates_do_not_search() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    let mut restricted = document("doc-restricted", &scope, "needle", "restricted");
    restricted.visibility = Visibility::Restricted;
    let mut tombstoned = document("doc-tombstoned", &scope, "needle", "tombstoned");
    tombstoned.visibility = Visibility::Tombstoned;
    tombstoned.source_status = SourceStatus::Tombstoned;

    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [
                document("doc-visible", &scope, "needle", "allowed"),
                restricted,
                tombstoned,
            ],
        )
        .unwrap();

    let search =
        index.search_candidates(&SearchQuery::new("workspace-a", "account-a", "needle", 10));

    assert_eq!(
        search.candidate_ids,
        vec![IndexCandidateId::new("doc-visible")]
    );
}

#[test]
fn authorized_results_are_derived_from_index_scope_and_citation() {
    let mut index = Bm25CandidateIndex::new();
    let scope_a = IndexScope::new("workspace-a", "account-a");
    let scope_b = IndexScope::new("workspace-b", "account-a");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope_a.clone(), 1, 1),
            [document("shared-doc", &scope_a, "needle", "workspace a")],
        )
        .unwrap();
    index
        .replace_scope(
            IndexGeneration::fresh(2, scope_b.clone(), 1, 1),
            [document("shared-doc", &scope_b, "needle", "workspace b")],
        )
        .unwrap();

    let candidate_ids = [IndexCandidateId::new("shared-doc")];
    let authorized = index.authorize_candidates(
        &SearchQuery::new("workspace-a", "account-a", "needle", 10),
        &candidate_ids,
    );

    assert_eq!(authorized.len(), 1);
    assert_eq!(authorized[0].workspace_id, "workspace-a");
    assert_eq!(authorized[0].citation_id, "citation-shared-doc");
}

#[test]
fn authorization_without_document_citation_cannot_generate_result() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    let mut uncited = document("doc-uncited", &scope, "needle", "uncited");
    uncited.citation_ref = None;
    index
        .replace_scope(IndexGeneration::fresh(1, scope.clone(), 1, 1), [uncited])
        .unwrap();

    let authorized = index.authorize_candidates(
        &SearchQuery::new("workspace-a", "account-a", "needle", 10),
        &[IndexCandidateId::new("doc-uncited")],
    );

    assert!(authorized.is_empty());
}

#[test]
fn visibility_revoke_marks_stale_and_removes_candidate_from_search() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    let candidate_id = IndexCandidateId::new("doc-1");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [document("doc-1", &scope, "needle", "visible")],
        )
        .unwrap();

    index
        .mark_visibility(&scope, &candidate_id, Visibility::Restricted)
        .unwrap();

    assert_eq!(index.generation(&scope).unwrap().status, IndexStatus::Stale);
    let search =
        index.search_candidates(&SearchQuery::new("workspace-a", "account-a", "needle", 10));
    assert!(search.candidate_ids.is_empty());

    let authorized = index.authorize_candidates(
        &SearchQuery::new("workspace-a", "account-a", "needle", 10),
        std::slice::from_ref(&candidate_id),
    );
    assert!(authorized.is_empty());
}

#[test]
fn tombstone_marks_cleanup_required_and_removes_source_candidates() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [
                document_with_source("doc-1", &scope, "source-a", "needle", "one"),
                document_with_source("doc-2", &scope, "source-a", "needle", "two"),
                document_with_source("doc-3", &scope, "source-b", "needle", "three"),
            ],
        )
        .unwrap();

    index.mark_source_tombstoned(&scope, "source-a").unwrap();

    let generation = index.generation(&scope).unwrap();
    assert_eq!(generation.status, IndexStatus::CleanupRequired);
    assert!(generation.requires_cleanup());

    let search =
        index.search_candidates(&SearchQuery::new("workspace-a", "account-a", "needle", 10));
    assert_eq!(search.candidate_ids, vec![IndexCandidateId::new("doc-3")]);
}

#[test]
fn resolve_citation_finds_visible_candidate_by_citation_id() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [document("doc-1", &scope, "needle", "visible body")],
        )
        .unwrap();

    let resolved = index.resolve_citation(&scope, "citation-doc-1");
    assert!(resolved.is_some());
    let result = resolved.unwrap();
    assert_eq!(result.citation_id, "citation-doc-1");
    assert_eq!(result.source_id, "source-1");
    assert_eq!(result.title, "needle");
    assert_eq!(result.snippet.as_deref(), Some("visible body"));
}

#[test]
fn resolve_citation_returns_none_for_restricted_or_tombstoned() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    let mut restricted = document("doc-restricted", &scope, "needle", "restricted");
    restricted.visibility = Visibility::Restricted;
    index
        .replace_scope(IndexGeneration::fresh(1, scope.clone(), 1, 1), [restricted])
        .unwrap();

    assert!(index
        .resolve_citation(&scope, "citation-doc-restricted")
        .is_none());
}

#[test]
fn failed_generation_records_failure_reason() {
    let mut index = Bm25CandidateIndex::new();
    let scope = IndexScope::new("workspace-a", "account-a");
    index
        .replace_scope(
            IndexGeneration::fresh(1, scope.clone(), 1, 1),
            [document("doc-1", &scope, "needle", "body")],
        )
        .unwrap();

    index.mark_failed(&scope, "wal append failed").unwrap();

    let generation = index.generation(&scope).unwrap();
    assert!(generation.failed());
    assert_eq!(
        generation.failure_reason.as_deref(),
        Some("wal append failed")
    );
}

fn document(id: &str, scope: &IndexScope, title: &str, body: &str) -> IndexedDocument {
    document_with_source(id, scope, "source-1", title, body)
}

fn document_with_source(
    id: &str,
    scope: &IndexScope,
    source_id: &str,
    title: &str,
    body: &str,
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
        visibility: Visibility::Visible,
        source_status: SourceStatus::Active,
        source_revision: 1,
        wiki_revision: 1,
    }
}
