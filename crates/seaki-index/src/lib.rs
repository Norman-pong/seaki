use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const SEARCH_RESULT_DISCLOSURE: &str = "candidate-ids-first";
pub const DEFAULT_SCHEMA_VERSION: &str = "seaki-index.bm25.v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexCandidateId(pub String);

impl IndexCandidateId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl fmt::Display for IndexCandidateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexScope {
    pub workspace_id: String,
    pub account_id: String,
}

impl IndexScope {
    pub fn new(workspace_id: impl Into<String>, account_id: impl Into<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            account_id: account_id.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexStatus {
    Fresh,
    Stale,
    CleanupRequired,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexGeneration {
    pub generation_id: u64,
    pub schema_version: String,
    pub workspace_id: String,
    pub account_id: String,
    pub source_revision: u64,
    pub wiki_revision: u64,
    pub status: IndexStatus,
    pub failure_reason: Option<String>,
}

impl IndexGeneration {
    pub fn fresh(
        generation_id: u64,
        scope: IndexScope,
        source_revision: u64,
        wiki_revision: u64,
    ) -> Self {
        Self::fresh_with_schema(
            generation_id,
            scope,
            DEFAULT_SCHEMA_VERSION,
            source_revision,
            wiki_revision,
        )
    }

    pub fn fresh_with_schema(
        generation_id: u64,
        scope: IndexScope,
        schema_version: impl Into<String>,
        source_revision: u64,
        wiki_revision: u64,
    ) -> Self {
        Self {
            generation_id,
            schema_version: schema_version.into(),
            workspace_id: scope.workspace_id,
            account_id: scope.account_id,
            source_revision,
            wiki_revision,
            status: IndexStatus::Fresh,
            failure_reason: None,
        }
    }

    pub fn scope(&self) -> IndexScope {
        IndexScope::new(self.workspace_id.clone(), self.account_id.clone())
    }

    pub const fn is_fresh(&self) -> bool {
        matches!(self.status, IndexStatus::Fresh)
    }

    pub const fn is_stale(&self) -> bool {
        matches!(self.status, IndexStatus::Stale)
    }

    pub const fn requires_cleanup(&self) -> bool {
        matches!(self.status, IndexStatus::CleanupRequired)
    }

    pub const fn failed(&self) -> bool {
        matches!(self.status, IndexStatus::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Restricted,
    Tombstoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    Active,
    Tombstoned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateKind {
    WikiPage,
    Claim,
    SourceFrame,
    MemoryNote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRangeUnit {
    Byte,
    Line,
    Page,
    Anchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRange {
    pub unit: SourceRangeUnit,
    pub start: u64,
    pub end: u64,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedCitationRef {
    pub citation_id: String,
    pub source_id: String,
    pub range: SourceRange,
    pub wiki_page_id: String,
    pub claim_id: String,
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedDocument {
    pub candidate_id: IndexCandidateId,
    pub workspace_id: String,
    pub account_id: String,
    pub source_id: String,
    pub citation_ref: Option<IndexedCitationRef>,
    pub kind: CandidateKind,
    pub title: String,
    pub body: String,
    pub visibility: Visibility,
    pub source_status: SourceStatus,
    pub source_revision: u64,
    pub wiki_revision: u64,
}

impl IndexedDocument {
    pub fn scope(&self) -> IndexScope {
        IndexScope::new(self.workspace_id.clone(), self.account_id.clone())
    }

    fn is_candidate_visible(&self) -> bool {
        self.visibility == Visibility::Visible && self.source_status == SourceStatus::Active
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub workspace_id: String,
    pub account_id: String,
    pub text: String,
    pub limit: usize,
}

impl SearchQuery {
    pub fn new(
        workspace_id: impl Into<String>,
        account_id: impl Into<String>,
        text: impl Into<String>,
        limit: usize,
    ) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            account_id: account_id.into(),
            text: text.into(),
            limit,
        }
    }

    pub fn scope(&self) -> IndexScope {
        IndexScope::new(self.workspace_id.clone(), self.account_id.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSearch {
    pub disclosure: &'static str,
    pub status: IndexStatus,
    pub candidate_ids: Vec<IndexCandidateId>,
}

impl CandidateSearch {
    fn empty(status: IndexStatus) -> Self {
        Self {
            disclosure: SEARCH_RESULT_DISCLOSURE,
            status,
            candidate_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedSearchResult {
    pub candidate_id: IndexCandidateId,
    pub workspace_id: String,
    pub account_id: String,
    pub source_id: String,
    pub citation_id: String,
    pub kind: CandidateKind,
    pub title: String,
    pub snippet: Option<String>,
    pub citation_refs: Vec<IndexedCitationRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexError {
    ScopeMismatch {
        candidate_id: IndexCandidateId,
        expected: IndexScope,
        actual: IndexScope,
    },
    UnknownScope {
        scope: IndexScope,
    },
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeMismatch {
                candidate_id,
                expected,
                actual,
            } => write!(
                f,
                "candidate {candidate_id} scope mismatch: expected {}/{}, got {}/{}",
                expected.workspace_id, expected.account_id, actual.workspace_id, actual.account_id
            ),
            Self::UnknownScope { scope } => write!(
                f,
                "index scope {}/{} has no generation",
                scope.workspace_id, scope.account_id
            ),
        }
    }
}

impl std::error::Error for IndexError {}

#[derive(Debug, Clone)]
pub struct Bm25CandidateIndex {
    documents: BTreeMap<ScopedCandidateKey, IndexedEntry>,
    generations: BTreeMap<IndexScope, IndexGeneration>,
}

impl Bm25CandidateIndex {
    pub fn new() -> Self {
        Self {
            documents: BTreeMap::new(),
            generations: BTreeMap::new(),
        }
    }

    pub fn replace_scope(
        &mut self,
        generation: IndexGeneration,
        documents: impl IntoIterator<Item = IndexedDocument>,
    ) -> Result<(), IndexError> {
        let scope = generation.scope();
        let mut entries = Vec::new();

        for document in documents {
            let actual = document.scope();
            if actual != scope {
                return Err(IndexError::ScopeMismatch {
                    candidate_id: document.candidate_id.clone(),
                    expected: scope.clone(),
                    actual,
                });
            }
            entries.push(IndexedEntry::new(document));
        }

        self.documents.retain(|key, _| key.scope != scope);
        for entry in entries {
            let key = ScopedCandidateKey::new(scope.clone(), entry.document.candidate_id.clone());
            self.documents.insert(key, entry);
        }
        self.generations.insert(scope, generation);
        Ok(())
    }

    pub fn generation(&self, scope: &IndexScope) -> Option<&IndexGeneration> {
        self.generations.get(scope)
    }

    pub fn search_candidates(&self, query: &SearchQuery) -> CandidateSearch {
        let scope = query.scope();
        let status = self
            .generations
            .get(&scope)
            .map(|generation| generation.status)
            .unwrap_or(IndexStatus::Stale);

        if query.limit == 0 {
            return CandidateSearch::empty(status);
        }

        let terms = tokenize(&query.text);
        if terms.is_empty() {
            return CandidateSearch::empty(status);
        }

        let documents = self
            .documents
            .values()
            .filter(|entry| {
                entry.document.scope() == scope && entry.document.is_candidate_visible()
            })
            .collect::<Vec<_>>();
        if documents.is_empty() {
            return CandidateSearch::empty(status);
        }

        let avg_doc_len = documents
            .iter()
            .map(|entry| entry.doc_len as f64)
            .sum::<f64>()
            / documents.len() as f64;
        let document_frequency = document_frequency(&documents, &terms);
        let mut scored = documents
            .into_iter()
            .filter_map(|entry| {
                let score = bm25_score(entry, &terms, &document_frequency, avg_doc_len);
                (score > 0.0).then(|| (entry.document.candidate_id.clone(), score))
            })
            .collect::<Vec<_>>();

        scored.sort_by(|(left_id, left_score), (right_id, right_score)| {
            right_score
                .total_cmp(left_score)
                .then_with(|| left_id.cmp(right_id))
        });

        CandidateSearch {
            disclosure: SEARCH_RESULT_DISCLOSURE,
            status,
            candidate_ids: scored
                .into_iter()
                .take(query.limit)
                .map(|(candidate_id, _)| candidate_id)
                .collect(),
        }
    }

    pub fn authorize_candidates(
        &self,
        query: &SearchQuery,
        candidate_ids: &[IndexCandidateId],
    ) -> Vec<AuthorizedSearchResult> {
        let scope = query.scope();
        candidate_ids
            .iter()
            .filter_map(|candidate_id| {
                let key = ScopedCandidateKey::new(scope.clone(), candidate_id.clone());
                let entry = self.documents.get(&key)?;
                if !entry.document.is_candidate_visible() {
                    return None;
                }
                let citation_ref = entry.document.citation_ref.as_ref()?;
                if citation_ref.source_id != entry.document.source_id {
                    return None;
                }

                Some(AuthorizedSearchResult {
                    candidate_id: candidate_id.clone(),
                    workspace_id: entry.document.workspace_id.clone(),
                    account_id: entry.document.account_id.clone(),
                    source_id: entry.document.source_id.clone(),
                    citation_id: citation_ref.citation_id.clone(),
                    kind: entry.document.kind.clone(),
                    title: entry.document.title.clone(),
                    snippet: snippet_for_query(&entry.document.body, &query.text),
                    citation_refs: vec![citation_ref.clone()],
                })
            })
            .collect()
    }

    pub fn mark_visibility(
        &mut self,
        scope: &IndexScope,
        candidate_id: &IndexCandidateId,
        visibility: Visibility,
    ) -> Result<(), IndexError> {
        self.ensure_scope(scope)?;
        let key = ScopedCandidateKey::new(scope.clone(), candidate_id.clone());
        if let Some(entry) = self.documents.get_mut(&key) {
            entry.document.visibility = visibility;
        }

        match visibility {
            Visibility::Visible => self.mark_status(scope, IndexStatus::Stale, None),
            Visibility::Restricted => self.mark_status(scope, IndexStatus::Stale, None),
            Visibility::Tombstoned => self.mark_status(scope, IndexStatus::CleanupRequired, None),
        }
    }

    pub fn mark_source_tombstoned(
        &mut self,
        scope: &IndexScope,
        source_id: &str,
    ) -> Result<(), IndexError> {
        self.ensure_scope(scope)?;
        for entry in self.documents.values_mut() {
            if entry.document.scope() == *scope && entry.document.source_id == source_id {
                entry.document.source_status = SourceStatus::Tombstoned;
                entry.document.visibility = Visibility::Tombstoned;
            }
        }
        self.mark_status(scope, IndexStatus::CleanupRequired, None)
    }

    pub fn mark_stale(&mut self, scope: &IndexScope) -> Result<(), IndexError> {
        self.ensure_scope(scope)?;
        self.mark_status(scope, IndexStatus::Stale, None)
    }

    pub fn resolve_citation(
        &self,
        scope: &IndexScope,
        citation_id: &str,
    ) -> Option<AuthorizedSearchResult> {
        let candidate = self.documents.values().find(|entry| {
            entry.document.scope() == *scope
                && entry.document.is_candidate_visible()
                && entry
                    .document
                    .citation_ref
                    .as_ref()
                    .is_some_and(|citation_ref| citation_ref.citation_id == citation_id)
        })?;

        let citation_ref = candidate.document.citation_ref.as_ref()?;
        if citation_ref.source_id != candidate.document.source_id {
            return None;
        }

        Some(AuthorizedSearchResult {
            candidate_id: candidate.document.candidate_id.clone(),
            workspace_id: candidate.document.workspace_id.clone(),
            account_id: candidate.document.account_id.clone(),
            source_id: candidate.document.source_id.clone(),
            citation_id: citation_ref.citation_id.clone(),
            kind: candidate.document.kind.clone(),
            title: candidate.document.title.clone(),
            snippet: Some(candidate.document.body.chars().take(160).collect()),
            citation_refs: vec![citation_ref.clone()],
        })
    }

    pub fn get_document(
        &self,
        scope: &IndexScope,
        candidate_id: &IndexCandidateId,
    ) -> Option<&IndexedDocument> {
        self.documents
            .get(&ScopedCandidateKey::new(
                scope.clone(),
                candidate_id.clone(),
            ))
            .map(|entry| &entry.document)
    }

    pub fn mark_failed(
        &mut self,
        scope: &IndexScope,
        reason: impl Into<String>,
    ) -> Result<(), IndexError> {
        self.ensure_scope(scope)?;
        self.mark_status(scope, IndexStatus::Failed, Some(reason.into()))
    }

    fn ensure_scope(&self, scope: &IndexScope) -> Result<(), IndexError> {
        if self.generations.contains_key(scope) {
            Ok(())
        } else {
            Err(IndexError::UnknownScope {
                scope: scope.clone(),
            })
        }
    }

    fn mark_status(
        &mut self,
        scope: &IndexScope,
        status: IndexStatus,
        failure_reason: Option<String>,
    ) -> Result<(), IndexError> {
        let Some(generation) = self.generations.get_mut(scope) else {
            return Err(IndexError::UnknownScope {
                scope: scope.clone(),
            });
        };
        generation.status = status;
        generation.failure_reason = failure_reason;
        Ok(())
    }
}

impl Default for Bm25CandidateIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ScopedCandidateKey {
    scope: IndexScope,
    candidate_id: IndexCandidateId,
}

impl ScopedCandidateKey {
    fn new(scope: IndexScope, candidate_id: IndexCandidateId) -> Self {
        Self {
            scope,
            candidate_id,
        }
    }
}

#[derive(Debug, Clone)]
struct IndexedEntry {
    document: IndexedDocument,
    term_frequency: BTreeMap<String, usize>,
    unique_terms: BTreeSet<String>,
    doc_len: usize,
}

impl IndexedEntry {
    fn new(document: IndexedDocument) -> Self {
        let terms = tokenize(&format!("{} {}", document.title, document.body));
        let mut term_frequency = BTreeMap::new();
        for term in terms {
            *term_frequency.entry(term).or_insert(0) += 1;
        }
        let doc_len = term_frequency.values().sum();
        let unique_terms = term_frequency.keys().cloned().collect();
        Self {
            document,
            term_frequency,
            unique_terms,
            doc_len,
        }
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            (!token.is_empty()).then_some(token)
        })
        .collect()
}

fn document_frequency(
    documents: &[&IndexedEntry],
    terms: &[String],
) -> BTreeMap<String, (usize, usize)> {
    let mut frequencies = BTreeMap::new();
    let unique_query_terms = terms.iter().map(String::as_str).collect::<BTreeSet<_>>();

    for term in unique_query_terms {
        let matches = documents
            .iter()
            .filter(|entry| entry.unique_terms.contains(term))
            .count();
        frequencies.insert(term.to_string(), (matches, documents.len()));
    }

    frequencies
}

fn bm25_score(
    entry: &IndexedEntry,
    terms: &[String],
    document_frequency: &BTreeMap<String, (usize, usize)>,
    avg_doc_len: f64,
) -> f64 {
    const K1: f64 = 1.5;
    const B: f64 = 0.75;

    if entry.doc_len == 0 || avg_doc_len == 0.0 {
        return 0.0;
    }

    let mut score = 0.0;
    let unique_terms = terms.iter().map(String::as_str).collect::<BTreeSet<_>>();
    for term in unique_terms {
        let Some(term_frequency) = entry.term_frequency.get(term) else {
            continue;
        };
        let Some((doc_frequency, document_count)) = document_frequency.get(term).copied() else {
            continue;
        };
        if document_count == 0 {
            continue;
        }
        let idf = ((document_count as f64 - doc_frequency as f64 + 0.5)
            / (doc_frequency as f64 + 0.5)
            + 1.0)
            .ln();
        let tf = *term_frequency as f64;
        let normalized_tf =
            tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * entry.doc_len as f64 / avg_doc_len));
        score += idf * normalized_tf;
    }

    score
}

fn snippet_for_query(body: &str, _query: &str) -> Option<String> {
    const MAX_SNIPPET_CHARS: usize = 160;

    let body = body.trim();
    if body.is_empty() {
        return None;
    }

    Some(body.chars().take(MAX_SNIPPET_CHARS).collect())
}

#[cfg(test)]
mod tests {
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
}
