//! MemoryStore: in-memory storage for MemoryItem with capacity limit and LRU eviction.

use crate::memory_item::{MemoryItem, MemoryKind, MemoryStatus};
use seaki_index::IndexScope;
use std::collections::HashMap;

#[derive(Debug)]
pub struct MemoryStore {
    items: HashMap<String, MemoryItem>,
    capacity_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryStoreError {
    NotFound(String),
    InvalidTransition {
        from: MemoryStatus,
        to: MemoryStatus,
    },
    CapacityExceeded,
    DuplicateId(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacityStatus {
    Healthy,
    Warning,
    Critical,
    Full,
}

impl MemoryStore {
    #[must_use]
    pub fn new(capacity_limit: usize) -> Self {
        Self {
            items: HashMap::new(),
            capacity_limit,
        }
    }

    /// 插入一个新的 memory item（必须为 Proposed 状态）。
    ///
    /// # Errors
    ///
    /// 当 `memory_id` 已存在时返回 [`MemoryStoreError::DuplicateId`]；
    /// 当容量已满且无法淘汰时返回 [`MemoryStoreError::CapacityExceeded`]。
    pub fn propose(&mut self, item: MemoryItem) -> Result<(), MemoryStoreError> {
        if self.items.contains_key(&item.memory_id) {
            return Err(MemoryStoreError::DuplicateId(item.memory_id));
        }
        self.evict_if_needed()?;
        self.items.insert(item.memory_id.clone(), item);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, memory_id: &str) -> Option<&MemoryItem> {
        self.items.get(memory_id)
    }

    /// 转换指定 memory item 的状态。
    ///
    /// # Errors
    ///
    /// 当 item 不存在时返回 [`MemoryStoreError::NotFound`]；
    /// 当状态转换不被允许时返回 [`MemoryStoreError::InvalidTransition`]。
    pub fn transition_status(
        &mut self,
        memory_id: &str,
        new_status: MemoryStatus,
    ) -> Result<(), MemoryStoreError> {
        let item = self
            .items
            .get_mut(memory_id)
            .ok_or_else(|| MemoryStoreError::NotFound(memory_id.to_string()))?;
        if !item.status.can_transition_to(new_status) {
            return Err(MemoryStoreError::InvalidTransition {
                from: item.status,
                to: new_status,
            });
        }
        item.status = new_status;
        Ok(())
    }

    #[must_use]
    pub fn items_by_status(&self, status: MemoryStatus) -> Vec<&MemoryItem> {
        self.items.values().filter(|i| i.status == status).collect()
    }

    #[must_use]
    pub fn items_by_kind(&self, kind: MemoryKind) -> Vec<&MemoryItem> {
        self.items.values().filter(|i| i.kind == kind).collect()
    }

    #[must_use]
    pub fn active_items_for_scope(&self, scope: &IndexScope) -> Vec<&MemoryItem> {
        self.items
            .values()
            .filter(|i| i.scope == *scope && i.status == MemoryStatus::Active)
            .collect()
    }

    /// 将已过期的项标记为 `Expired`，并返回被标记的项。
    pub fn prune_expired(&mut self, now: u64) -> Vec<MemoryItem> {
        let mut expired = Vec::new();
        for item in self.items.values_mut() {
            if let Some(expires_at) = item.expires_at {
                if now >= expires_at && item.status != MemoryStatus::Expired {
                    item.status = MemoryStatus::Expired;
                    expired.push(item.clone());
                }
            }
        }
        expired
    }

    #[must_use]
    pub fn check_capacity(&self) -> CapacityStatus {
        if self.capacity_limit == 0 {
            return CapacityStatus::Full;
        }
        let ratio = self.items.len() as f64 / self.capacity_limit as f64;
        if ratio >= 1.0 {
            CapacityStatus::Full
        } else if ratio > 0.95 {
            CapacityStatus::Critical
        } else if ratio > 0.80 {
            CapacityStatus::Warning
        } else {
            CapacityStatus::Healthy
        }
    }

    /// 按 LRU（以 `proposed_at` 近似）淘汰最旧的非关键项到 `Archived`，
    /// 直到容量低于上限。若全部项均为关键状态（Active / Approved）且无法淘汰，
    /// 则返回容量超限错误。
    ///
    /// # Errors
    ///
    /// 当无法淘汰任何项以腾出空间时返回 [`MemoryStoreError::CapacityExceeded`]。
    pub fn evict_if_needed(&mut self) -> Result<(), MemoryStoreError> {
        if self.items.len() < self.capacity_limit {
            return Ok(());
        }

        // 寻找最旧的、可以被淘汰的项（非 Active / Approved）
        let candidate = self
            .items
            .values()
            .filter(|i| i.status != MemoryStatus::Active && i.status != MemoryStatus::Approved)
            .min_by_key(|i| i.proposed_at)
            .map(|i| i.memory_id.clone());

        if let Some(id) = candidate {
            if let Some(item) = self.items.get_mut(&id) {
                item.status = MemoryStatus::Archived;
            }
            return Ok(());
        }

        // 如果所有项都是 Active / Approved，尝试淘汰最旧的 Approved
        let candidate = self
            .items
            .values()
            .filter(|i| i.status == MemoryStatus::Approved)
            .min_by_key(|i| i.proposed_at)
            .map(|i| i.memory_id.clone());

        if let Some(id) = candidate {
            if let Some(item) = self.items.get_mut(&id) {
                item.status = MemoryStatus::Archived;
            }
            return Ok(());
        }

        Err(MemoryStoreError::CapacityExceeded)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
