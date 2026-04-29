//! Session search: redacted manifest, TTL/scope, candidate query.

use crate::redaction::RedactedSessionManifest;
use seaki_index::{
    Bm25CandidateIndex, CandidateKind, IndexCandidateId, IndexGeneration, IndexScope,
    IndexedDocument, SearchQuery, SourceStatus, Visibility,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSearchCandidate {
    pub session_id: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSearchError {
    Index(seaki_index::IndexError),
}

impl From<seaki_index::IndexError> for SessionSearchError {
    fn from(err: seaki_index::IndexError) -> Self {
        SessionSearchError::Index(err)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIndexStatus {
    Active,
    Expired,
}

pub struct SessionManifestEntry {
    pub manifest: RedactedSessionManifest,
    pub status: SessionIndexStatus,
    pub expired_at: Option<u64>,
    pub delete_after: Option<u64>,
}

/// 后台清理动作，由调用方（如 CoreLedger）执行物理删除并生成 AuditEvent。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCleanupAction {
    MarkExpired {
        session_id: String,
    },
    PhysicallyDelete {
        session_id: String,
        scope: IndexScope,
    },
}

/// 管理 session manifest 的内存存储与 BM25 索引交互。
pub struct SessionSearchIndex {
    entries: HashMap<String, SessionManifestEntry>,
    generation_counter: u64,
}

impl SessionSearchIndex {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            generation_counter: 1,
        }
    }

    /// 将脱敏后的 session manifest 加入 BM25 索引。
    pub fn index_redacted_session(
        &mut self,
        manifest: RedactedSessionManifest,
        index: &mut Bm25CandidateIndex,
    ) -> Result<(), seaki_index::IndexError> {
        let delete_after = manifest.redacted_at.saturating_add(manifest.ttl_seconds);
        self.entries.insert(
            manifest.session_id.clone(),
            SessionManifestEntry {
                manifest: manifest.clone(),
                status: SessionIndexStatus::Active,
                expired_at: None,
                delete_after: Some(delete_after),
            },
        );
        self.rebuild_scope(index, &manifest.scope)
    }

    /// 搜索会话。使用与 wiki source 分离的 session scope，避免 replace_scope 互相覆盖。
    pub fn search_sessions(
        &self,
        query_text: &str,
        scope: &IndexScope,
        index: &Bm25CandidateIndex,
        limit: usize,
    ) -> Result<Vec<SessionSearchCandidate>, SessionSearchError> {
        let session_scope = session_scope(scope);
        let query = SearchQuery::new(
            session_scope.workspace_id.clone(),
            session_scope.account_id.clone(),
            query_text,
            limit,
        );
        let search = index.search_candidates(&query);

        let mut candidates = Vec::new();
        for candidate_id in &search.candidate_ids {
            if let Some(doc) = index.get_document(&session_scope, candidate_id) {
                if doc.visibility == Visibility::Visible
                    && doc.source_status == SourceStatus::Active
                {
                    candidates.push(SessionSearchCandidate {
                        session_id: candidate_id.0.clone(),
                        snippet: Some(doc.body.chars().take(160).collect()),
                    });
                }
            }
        }
        Ok(candidates)
    }

    /// TTL 过期条目先标记 `expired`，7 天后物理删除并返回清理动作。
    /// 调用方应在收到 `PhysicallyDelete` 后生成 AuditEvent。
    pub fn cleanup_expired_sessions(
        &mut self,
        now: u64,
        index: &mut Bm25CandidateIndex,
    ) -> Result<Vec<SessionCleanupAction>, seaki_index::IndexError> {
        const GRACE_PERIOD_SECONDS: u64 = 7 * 24 * 60 * 60;

        let mut actions = Vec::new();
        let mut scopes_to_rebuild = std::collections::HashSet::new();

        // 第一轮扫描：标记过期
        for (session_id, entry) in self.entries.iter_mut() {
            if entry.status == SessionIndexStatus::Active {
                if let Some(delete_after) = entry.delete_after {
                    if now >= delete_after {
                        entry.status = SessionIndexStatus::Expired;
                        entry.expired_at = Some(now);
                        actions.push(SessionCleanupAction::MarkExpired {
                            session_id: session_id.clone(),
                        });
                        scopes_to_rebuild.insert(entry.manifest.scope.clone());
                    }
                }
            }
        }

        // 第二轮扫描：物理删除已过期超过 grace period 的条目
        let mut to_remove = Vec::new();
        for (session_id, entry) in &self.entries {
            if entry.status == SessionIndexStatus::Expired {
                if let Some(expired_at) = entry.expired_at {
                    if now >= expired_at.saturating_add(GRACE_PERIOD_SECONDS) {
                        to_remove.push(session_id.clone());
                        actions.push(SessionCleanupAction::PhysicallyDelete {
                            session_id: session_id.clone(),
                            scope: entry.manifest.scope.clone(),
                        });
                        scopes_to_rebuild.insert(entry.manifest.scope.clone());
                    }
                }
            }
        }

        for session_id in &to_remove {
            self.entries.remove(session_id);
        }

        // 重建受影响 scope 的索引（物理删除后从索引中移除）
        for scope in scopes_to_rebuild {
            self.rebuild_scope(index, &scope)?;
        }

        Ok(actions)
    }

    pub fn get_entry(&self, session_id: &str) -> Option<&SessionManifestEntry> {
        self.entries.get(session_id)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    fn rebuild_scope(
        &mut self,
        index: &mut Bm25CandidateIndex,
        scope: &IndexScope,
    ) -> Result<(), seaki_index::IndexError> {
        let session_scope = session_scope(scope);
        let docs: Vec<IndexedDocument> = self
            .entries
            .values()
            .filter(|e| e.manifest.scope == *scope && e.status == SessionIndexStatus::Active)
            .map(|e| manifest_to_document(&e.manifest))
            .collect();

        let generation =
            IndexGeneration::fresh(self.generation_counter, session_scope.clone(), 1, 1);
        self.generation_counter += 1;
        index.replace_scope(generation, docs)
    }
}

impl Default for SessionSearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// session scope 与 wiki source scope 分离，减小 replace_scope 粒度。
pub fn session_scope(base: &IndexScope) -> IndexScope {
    IndexScope::new(
        base.workspace_id.clone(),
        format!("{}:session", base.account_id),
    )
}

fn manifest_to_document(manifest: &RedactedSessionManifest) -> IndexedDocument {
    let session_scope = session_scope(&manifest.scope);
    IndexedDocument {
        candidate_id: IndexCandidateId::new(&manifest.session_id),
        workspace_id: manifest.scope.workspace_id.clone(),
        account_id: session_scope.account_id.clone(),
        source_id: format!("session:{}", manifest.session_id),
        citation_ref: None, // session 不可被 citation 直接引用
        kind: CandidateKind::MemoryNote,
        title: manifest.summary.clone(),
        body: manifest.summary.clone(),
        visibility: Visibility::Visible,
        source_status: SourceStatus::Active,
        source_revision: 1,
        wiki_revision: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redaction::RedactedSessionManifest;
    use seaki_index::IndexScope;

    fn scope() -> IndexScope {
        IndexScope::new("workspace-a", "account-a")
    }

    fn manifest(id: &str, summary: &str) -> RedactedSessionManifest {
        RedactedSessionManifest::new(id, summary, scope(), format!("ref://{}", id))
    }

    #[test]
    fn index_does_not_save_original_transcript() {
        let mut bm25 = Bm25CandidateIndex::new();
        let mut sessions = SessionSearchIndex::new();

        let m = manifest("s-1", "user asked about rust");
        sessions
            .index_redacted_session(m.clone(), &mut bm25)
            .unwrap();

        // 索引中只存在 summary，不存在 transcript ref
        let doc = bm25
            .get_document(&session_scope(&scope()), &IndexCandidateId::new("s-1"))
            .unwrap();
        assert!(doc.body.contains("user asked about rust"));
        assert!(!doc.body.contains("ref://"));
    }

    #[test]
    fn search_returns_session_candidates() {
        let mut bm25 = Bm25CandidateIndex::new();
        let mut sessions = SessionSearchIndex::new();

        sessions
            .index_redacted_session(manifest("s-1", "rust ownership questions"), &mut bm25)
            .unwrap();
        sessions
            .index_redacted_session(manifest("s-2", "python async questions"), &mut bm25)
            .unwrap();

        let results = sessions
            .search_sessions("rust", &scope(), &bm25, 10)
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "s-1");
        assert!(results[0].snippet.as_deref().unwrap().contains("rust"));
    }

    #[test]
    fn ttl_expired_marked_then_deleted_after_grace_period() {
        let mut bm25 = Bm25CandidateIndex::new();
        let mut sessions = SessionSearchIndex::new();

        let mut m = manifest("s-1", "question");
        m.redacted_at = 0;
        m.ttl_seconds = 10; // 10 秒后过期
        sessions.index_redacted_session(m, &mut bm25).unwrap();

        // 第 1 阶段：TTL 刚到期 -> 标记 expired
        let actions = sessions.cleanup_expired_sessions(15, &mut bm25).unwrap();
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0], SessionCleanupAction::MarkExpired { session_id } if session_id == "s-1")
        );

        // 索引中应该已经不可搜索（Active 被重建）
        let results = sessions
            .search_sessions("question", &scope(), &bm25, 10)
            .unwrap();
        assert!(results.is_empty());

        // 第 2 阶段：7 天 grace period 后 -> 物理删除
        let actions = sessions
            .cleanup_expired_sessions(15 + 7 * 24 * 60 * 60 + 1, &mut bm25)
            .unwrap();
        assert!(
            matches!(&actions[0], SessionCleanupAction::PhysicallyDelete { session_id, .. } if session_id == "s-1")
        );
        assert_eq!(sessions.entry_count(), 0);
    }

    #[test]
    fn session_scope_is_isolated_from_wiki_scope() {
        let base = IndexScope::new("ws", "ac");
        let sess = session_scope(&base);
        assert_eq!(sess.workspace_id, "ws");
        assert_eq!(sess.account_id, "ac:session");
    }
}
