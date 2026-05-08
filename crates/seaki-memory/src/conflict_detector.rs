//! ConflictDetector: stub implementation for detecting memory vs wiki/source conflicts.
//!
//! Uses simple keyword overlap heuristics.

use crate::memory_item::MemoryItem;

pub struct ConflictDetector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictReport {
    pub memory_id: String,
    pub conflict_type: ConflictType,
    pub conflicting_keywords: Vec<String>,
    pub recommendation: ConflictResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictType {
    KeywordOverlap,
    Contradiction,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    DowngradeToStale,
    DowngradeToHint,
    Reject,
    Merge,
}

impl ConflictDetector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 检测 memory 是否与 wiki claim 关键词存在冲突。
    #[must_use]
    pub fn detect_conflicts(
        &self,
        memory: &MemoryItem,
        wiki_claim_keywords: &[String],
    ) -> Vec<ConflictReport> {
        let content_lower = memory.content.to_lowercase();
        let mut conflicts = Vec::new();
        let mut matched = Vec::new();

        for kw in wiki_claim_keywords {
            if content_lower.contains(&kw.to_lowercase()) {
                matched.push(kw.clone());
            }
        }

        if !matched.is_empty() {
            let recommendation = if matched.len() > 2 {
                ConflictResolution::Reject
            } else {
                ConflictResolution::DowngradeToStale
            };

            conflicts.push(ConflictReport {
                memory_id: memory.memory_id.clone(),
                conflict_type: ConflictType::KeywordOverlap,
                conflicting_keywords: matched,
                recommendation,
            });
        }

        conflicts
    }
}

impl Default for ConflictDetector {
    fn default() -> Self {
        Self::new()
    }
}
