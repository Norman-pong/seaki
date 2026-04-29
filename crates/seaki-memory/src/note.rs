//! Project note: CRUD, BM25 indexing, `source_checking`.

use seaki_index::{
    Bm25CandidateIndex, CandidateKind, IndexCandidateId, IndexGeneration, IndexScope,
    IndexedDocument, SearchQuery, SourceStatus, Visibility,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectNote {
    pub note_id: String,
    pub title: String,
    pub content: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub scope: IndexScope,
    pub status: NoteStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteStatus {
    Proposed,
    Scanning,
    SourceChecking,
    Approved,
    Active,
    Conflict,
    Stale,
}

impl NoteStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            NoteStatus::Proposed => "proposed",
            NoteStatus::Scanning => "scanning",
            NoteStatus::SourceChecking => "source_checking",
            NoteStatus::Approved => "approved",
            NoteStatus::Active => "active",
            NoteStatus::Conflict => "conflict",
            NoteStatus::Stale => "stale",
        }
    }

    #[must_use]
    pub fn can_transition_to(self, target: NoteStatus) -> bool {
        matches!(
            (self, target),
            (NoteStatus::Proposed, NoteStatus::Scanning)
                | (NoteStatus::Scanning, NoteStatus::SourceChecking)
                | (
                    NoteStatus::SourceChecking,
                    NoteStatus::Approved | NoteStatus::Conflict
                )
                | (NoteStatus::Approved, NoteStatus::Active)
                | (NoteStatus::Active | NoteStatus::Conflict, NoteStatus::Stale)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSearchResult {
    pub note_id: String,
    pub title: String,
    pub snippet: Option<String>,
    pub status: NoteStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteStoreError {
    NotFound(String),
    InvalidStatusTransition { from: NoteStatus, to: NoteStatus },
}

/// 内存中的 note 存储，负责 CRUD 与 BM25 索引交互。
pub struct NoteStore {
    notes: HashMap<String, ProjectNote>,
    generation_counter: u64,
}

impl NoteStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            notes: HashMap::new(),
            generation_counter: 1,
        }
    }

    pub fn create_note(&mut self, title: String, content: &str, scope: &IndexScope) -> ProjectNote {
        let note_id = format!("note-{}", next_id());
        let now = current_timestamp();
        let note = ProjectNote {
            note_id: note_id.clone(),
            title,
            content: content.to_string(),
            created_at: now,
            updated_at: now,
            scope: scope.clone(),
            status: NoteStatus::Proposed,
        };
        self.notes.insert(note_id, note.clone());
        note
    }

    /// 更新指定 note 的内容。
    ///
    /// # Errors
    ///
    /// 当 note 不存在时返回 [`NoteStoreError::NotFound`]。
    pub fn update_note(
        &mut self,
        note_id: &str,
        content: &str,
    ) -> Result<ProjectNote, NoteStoreError> {
        let note = self
            .notes
            .get_mut(note_id)
            .ok_or_else(|| NoteStoreError::NotFound(note_id.to_string()))?;
        note.content = content.to_string();
        note.updated_at = current_timestamp();
        Ok(note.clone())
    }

    /// 删除指定 note。
    ///
    /// # Errors
    ///
    /// 当 note 不存在时返回 [`NoteStoreError::NotFound`]。
    pub fn delete_note(&mut self, note_id: &str) -> Result<(), NoteStoreError> {
        self.notes
            .remove(note_id)
            .ok_or_else(|| NoteStoreError::NotFound(note_id.to_string()))?;
        Ok(())
    }

    #[must_use]
    pub fn note(&self, note_id: &str) -> Option<&ProjectNote> {
        self.notes.get(note_id)
    }

    /// 将 note 的状态转换为目标状态。
    ///
    /// # Errors
    ///
    /// 当 note 不存在时返回 [`NoteStoreError::NotFound`]；
    /// 当状态转换不被允许时返回 [`NoteStoreError::InvalidStatusTransition`]。
    pub fn transition_status(
        &mut self,
        note_id: &str,
        new_status: NoteStatus,
    ) -> Result<ProjectNote, NoteStoreError> {
        let note = self
            .notes
            .get_mut(note_id)
            .ok_or_else(|| NoteStoreError::NotFound(note_id.to_string()))?;
        if !note.status.can_transition_to(new_status) {
            return Err(NoteStoreError::InvalidStatusTransition {
                from: note.status,
                to: new_status,
            });
        }
        note.status = new_status;
        note.updated_at = current_timestamp();
        Ok(note.clone())
    }

    /// 将当前 scope 下的所有 note 重建到 BM25 索引（memory scope）。
    ///
    /// # Errors
    ///
    /// 当索引替换失败时返回 [`seaki_index::IndexError`]。
    pub fn rebuild_index(
        &mut self,
        index: &mut Bm25CandidateIndex,
        scope: &IndexScope,
    ) -> Result<(), seaki_index::IndexError> {
        let memory_scope = memory_scope(scope);
        let docs: Vec<IndexedDocument> = self
            .notes
            .values()
            .filter(|n| n.scope == *scope)
            .map(note_to_document)
            .collect();
        let generation = IndexGeneration::fresh(self.generation_counter, memory_scope, 1, 1);
        self.generation_counter += 1;
        index.replace_scope(generation, docs)
    }

    /// 使用 BM25 搜索 note。note 使用独立的 memory scope，不与 wiki source 冲突。
    #[must_use]
    pub fn search_notes(
        &self,
        query_text: &str,
        scope: &IndexScope,
        index: &Bm25CandidateIndex,
        limit: usize,
    ) -> Vec<NoteSearchResult> {
        let memory_scope = memory_scope(scope);
        let query = SearchQuery::new(
            memory_scope.workspace_id.clone(),
            memory_scope.account_id.clone(),
            query_text,
            limit,
        );
        let search = index.search_candidates(&query);

        search
            .candidate_ids
            .iter()
            .filter_map(|id| {
                let doc = index.document(&memory_scope, id)?;
                if doc.visibility != Visibility::Visible
                    || doc.source_status != SourceStatus::Active
                {
                    return None;
                }
                let note = self.notes.get(&id.0)?;
                Some(NoteSearchResult {
                    note_id: note.note_id.clone(),
                    title: note.title.clone(),
                    snippet: Some(doc.body.chars().take(160).collect()),
                    status: note.status,
                })
            })
            .collect()
    }

    /// 最小 `source_checking：检测` note 内容与 wiki claim 关键词/引用重叠。
    /// 冲突则标记 `NoteStatus::Conflict` 并阻止进入 `Approved`。
    /// 返回 true 表示检测到冲突。
    ///
    /// # Errors
    ///
    /// 当 note 不存在时返回 [`NoteStoreError::NotFound`]。
    ///
    /// # Panics
    ///
    /// 当 note 在获取后意外从存储中消失时可能 panic（理论上不应发生）。
    pub fn check_source_conflicts(
        &mut self,
        note_id: &str,
        wiki_claim_keywords: &[String],
    ) -> Result<bool, NoteStoreError> {
        let note = self
            .notes
            .get(note_id)
            .ok_or_else(|| NoteStoreError::NotFound(note_id.to_string()))?;

        let content_lower = note.content.to_lowercase();
        let has_conflict = wiki_claim_keywords
            .iter()
            .any(|kw| content_lower.contains(&kw.to_lowercase()));

        if has_conflict {
            // 以 wiki/source 为准，memory 降级为 Conflict
            let note = self.notes.get_mut(note_id).unwrap();
            note.status = NoteStatus::Conflict;
            note.updated_at = current_timestamp();
        }

        Ok(has_conflict)
    }

    #[must_use]
    pub fn note_count(&self) -> usize {
        self.notes.len()
    }
}

impl Default for NoteStore {
    fn default() -> Self {
        Self::new()
    }
}

/// memory scope 与 wiki source scope 分离。
#[must_use]
pub fn memory_scope(base: &IndexScope) -> IndexScope {
    IndexScope::new(
        base.workspace_id.clone(),
        format!("{}:memory", base.account_id),
    )
}

fn note_to_document(note: &ProjectNote) -> IndexedDocument {
    let memory_scope = memory_scope(&note.scope);
    IndexedDocument {
        candidate_id: IndexCandidateId::new(&note.note_id),
        workspace_id: note.scope.workspace_id.clone(),
        account_id: memory_scope.account_id.clone(),
        source_id: format!("memory:{}", note.note_id),
        citation_ref: None, // note 不可被 citation 直接引用
        kind: CandidateKind::MemoryNote,
        title: note.title.clone(),
        body: note.content.clone(),
        visibility: Visibility::Visible,
        source_status: SourceStatus::Active,
        source_revision: 1,
        wiki_revision: 1,
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn next_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}
