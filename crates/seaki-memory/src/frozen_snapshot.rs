//! FrozenMemorySnapshot: session 启动时生成的不可变 memory snapshot。

use crate::{MemoryKind, MemoryStore, TrustLevel};
use seaki_index::IndexScope;

/// Session 启动时生成的 memory snapshot，中途不可变。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenMemorySnapshot {
    pub session_id: String,
    pub created_at: u64,
    pub scope: IndexScope,
    pub user_memories: Vec<SnapshotMemoryEntry>,
    pub project_memories: Vec<SnapshotMemoryEntry>,
    pub total_items: usize,
    pub total_bytes: usize,
}

/// Snapshot 中的单个 memory 条目（精简版，不含完整 provenance）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotMemoryEntry {
    pub memory_id: String,
    pub kind: MemoryKind,
    pub content: String,
    pub trust_level: TrustLevel,
    pub source_citation: Option<String>,
}

pub struct FrozenSnapshotBuilder<'a> {
    store: &'a MemoryStore,
}

impl<'a> FrozenSnapshotBuilder<'a> {
    #[must_use]
    pub fn new(store: &'a MemoryStore) -> Self {
        Self { store }
    }

    /// 为指定 scope 和 session 生成 frozen snapshot。
    /// 只选取 status == Active 且 trust_level >= min_trust_level 的 items。
    pub fn build(
        &self,
        session_id: &str,
        scope: &IndexScope,
        min_trust_level: TrustLevel,
        now: u64,
    ) -> FrozenMemorySnapshot {
        let active_items = self.store.active_items_for_scope(scope);

        let mut user_memories = Vec::new();
        let mut project_memories = Vec::new();
        let mut total_bytes = 0usize;

        for item in active_items {
            if !trust_level_satisfies(item.trust_level, min_trust_level) {
                continue;
            }

            if let Some(expires_at) = item.expires_at {
                if expires_at <= now {
                    continue;
                }
            }

            total_bytes += item.content.len();

            let entry = SnapshotMemoryEntry {
                memory_id: item.memory_id.clone(),
                kind: item.kind,
                content: item.content.clone(),
                trust_level: item.trust_level,
                source_citation: item.source_citation.clone(),
            };

            match item.kind {
                MemoryKind::UserPreference => user_memories.push(entry),
                MemoryKind::ProjectConvention
                | MemoryKind::WorkflowPattern
                | MemoryKind::SafetyRule
                | MemoryKind::DerivedFact => project_memories.push(entry),
            }
        }

        let total_items = user_memories.len() + project_memories.len();

        FrozenMemorySnapshot {
            session_id: session_id.to_string(),
            created_at: now,
            scope: scope.clone(),
            user_memories,
            project_memories,
            total_items,
            total_bytes,
        }
    }
}

fn trust_level_satisfies(level: TrustLevel, min: TrustLevel) -> bool {
    (level as u8) >= (min as u8)
}
