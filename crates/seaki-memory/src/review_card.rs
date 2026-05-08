//! ReviewCard: reviewable memory card with forgetting-curve scheduling state.

use crate::memory_item::{MemoryItem, MemoryKind};
use seaki_index::IndexScope;

/// 可复习的记忆卡片，基于 MemoryItem 构建，但独立管理复习状态。
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewCard {
    pub card_id: String,
    pub memory_id: Option<String>,
    pub scope: IndexScope,
    pub question: String,
    pub answer: String,
    pub source: Option<String>,
    pub created_at: u64,
    pub last_reviewed_at: Option<u64>,
    pub stability_days: f64,
    pub retention_threshold: f64,
    pub next_review_at: u64,
    pub review_count: u32,
    pub difficulty: CardDifficulty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardDifficulty {
    Easy,
    Medium,
    Hard,
    Critical,
}

impl CardDifficulty {
    #[must_use]
    pub fn default_threshold(&self) -> f64 {
        match self {
            CardDifficulty::Easy => 0.65,
            CardDifficulty::Medium => 0.72,
            CardDifficulty::Hard => 0.80,
            CardDifficulty::Critical => 0.90,
        }
    }

    #[must_use]
    pub fn initial_stability(&self) -> f64 {
        match self {
            CardDifficulty::Easy => 2.0,
            CardDifficulty::Medium => 1.0,
            CardDifficulty::Hard => 0.5,
            CardDifficulty::Critical => 0.3,
        }
    }
}

impl ReviewCard {
    /// 从 MemoryItem 构建 ReviewCard。
    #[must_use]
    pub fn from_memory_item(item: &MemoryItem, now: u64) -> Self {
        let difficulty = match item.kind {
            MemoryKind::UserPreference => CardDifficulty::Easy,
            MemoryKind::ProjectConvention => CardDifficulty::Medium,
            MemoryKind::WorkflowPattern => CardDifficulty::Medium,
            MemoryKind::SafetyRule => CardDifficulty::Critical,
            MemoryKind::DerivedFact => CardDifficulty::Hard,
        };
        let stability = difficulty.initial_stability();
        let threshold = difficulty.default_threshold();
        let next_review = now.saturating_add((stability * 86400.0) as u64);

        Self {
            card_id: format!("card_{}", item.memory_id),
            memory_id: Some(item.memory_id.clone()),
            scope: item.scope.clone(),
            question: item.content.clone(),
            answer: item.content.clone(),
            source: item.source_citation.clone(),
            created_at: item.proposed_at,
            last_reviewed_at: None,
            stability_days: stability,
            retention_threshold: threshold,
            next_review_at: next_review,
            review_count: 0,
            difficulty,
        }
    }
}
