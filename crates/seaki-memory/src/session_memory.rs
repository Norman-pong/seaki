//! SessionMemoryManager: 管理 session 与 memory 的关系。

use crate::{
    FrozenMemorySnapshot, FrozenSnapshotBuilder, MemoryItem, MemoryStore, MemoryStoreError,
    TrustLevel,
};
use seaki_index::IndexScope;
use std::collections::HashMap;

pub struct SessionMemoryManager {
    snapshots: HashMap<String, FrozenMemorySnapshot>,
    mid_session_writes: HashMap<String, Vec<MemoryItem>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionMemoryError {
    SessionNotFound(String),
    SnapshotAlreadyExists(String),
    StoreError(MemoryStoreError),
}

impl From<MemoryStoreError> for SessionMemoryError {
    fn from(err: MemoryStoreError) -> Self {
        SessionMemoryError::StoreError(err)
    }
}

impl SessionMemoryManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            mid_session_writes: HashMap::new(),
        }
    }

    /// Session 启动时生成 frozen snapshot。
    pub fn start_session(
        &mut self,
        session_id: &str,
        scope: &IndexScope,
        store: &MemoryStore,
        min_trust_level: TrustLevel,
        now: u64,
    ) -> &FrozenMemorySnapshot {
        if !self.snapshots.contains_key(session_id) {
            let builder = FrozenSnapshotBuilder::new(store);
            let snapshot = builder.build(session_id, scope, min_trust_level, now);
            self.snapshots.insert(session_id.to_string(), snapshot);
            self.mid_session_writes
                .entry(session_id.to_string())
                .or_default();
        }
        self.snapshots.get(session_id).unwrap()
    }

    /// Session 中途写入 memory（不热替换当前 snapshot）。
    pub fn write_during_session(
        &mut self,
        session_id: &str,
        item: MemoryItem,
    ) -> Result<(), SessionMemoryError> {
        let writes = self
            .mid_session_writes
            .get_mut(session_id)
            .ok_or_else(|| SessionMemoryError::SessionNotFound(session_id.to_string()))?;
        writes.push(item);
        Ok(())
    }

    /// Session 结束时，将 mid-session writes 持久化到 store。
    pub fn end_session(
        &mut self,
        session_id: &str,
        store: &mut MemoryStore,
    ) -> Result<Vec<MemoryItem>, SessionMemoryError> {
        let writes = self
            .mid_session_writes
            .remove(session_id)
            .ok_or_else(|| SessionMemoryError::SessionNotFound(session_id.to_string()))?;

        for item in &writes {
            store.propose(item.clone())?;
        }

        self.snapshots.remove(session_id);

        Ok(writes)
    }

    /// 获取指定 session 的 snapshot（只读）。
    #[must_use]
    pub fn snapshot(&self, session_id: &str) -> Option<&FrozenMemorySnapshot> {
        self.snapshots.get(session_id)
    }

    /// 获取指定 session 的中途写入（只读）。
    #[must_use]
    pub fn mid_session_writes(&self, session_id: &str) -> Option<&Vec<MemoryItem>> {
        self.mid_session_writes.get(session_id)
    }

    /// 清理已结束 session 的快照和写入记录。
    pub fn cleanup_session(&mut self, session_id: &str) {
        self.snapshots.remove(session_id);
        self.mid_session_writes.remove(session_id);
    }
}

impl Default for SessionMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}
