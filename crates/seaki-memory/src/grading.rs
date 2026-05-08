//! GradingEngine: SM-2-style grading feedback that adjusts stability_days.

use crate::retention::RetentionScheduler;
use crate::review_card::{CardDifficulty, ReviewCard};

/// 复习 grading 反馈，调整 stability_days。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grade {
    Again,
    Hard,
    Good,
    Easy,
}

/// Grading 后卡片状态的更新结果。
#[derive(Debug, Clone, PartialEq)]
pub struct GradingResult {
    pub new_stability_days: f64,
    pub new_next_review_at: u64,
    pub review_count: u32,
    pub recommended_action: GradingAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradingAction {
    Continue,
    ReviewSoon,
    RelinkToSource,
    Suspend,
}

pub struct GradingEngine;

impl GradingEngine {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 对卡片进行 grading，返回更新后的状态。
    ///
    /// SM-2 风格简化算法：
    /// - again: stability *= 0.3, 下次 10 分钟后
    /// - hard:  stability *= 0.8
    /// - good:  stability *= 1.5 + (0.1 * review_count)
    /// - easy:  stability *= 2.5 + (0.2 * review_count)
    ///
    /// Critical 难度卡片的 again 惩罚更重（0.15）。
    /// 反复答错（again 且 review_count >= 3）推荐回链 source。
    /// stability < 0.1 天推荐短期内再次复习。
    #[must_use]
    pub fn grade(&self, card: &ReviewCard, grade: Grade, now: u64) -> GradingResult {
        let multiplier = match (card.difficulty, grade) {
            (CardDifficulty::Critical, Grade::Again) => 0.15,
            (_, Grade::Again) => 0.3,
            (_, Grade::Hard) => 0.8,
            (_, Grade::Good) => 1.5 + 0.1 * f64::from(card.review_count),
            (CardDifficulty::Easy, Grade::Easy) => 3.0 + 0.2 * f64::from(card.review_count),
            (_, Grade::Easy) => 2.5 + 0.2 * f64::from(card.review_count),
        };

        let raw_stability = card.stability_days * multiplier;
        let new_stability = if raw_stability.is_finite() {
            raw_stability.clamp(0.01, 3650.0)
        } else {
            3650.0
        };

        let next_review = if grade == Grade::Again {
            now + 600
        } else {
            RetentionScheduler::next_review_at(now, new_stability)
        };

        let new_review_count = card.review_count.saturating_add(1);

        let action = if grade == Grade::Again && card.review_count >= 3 {
            GradingAction::RelinkToSource
        } else if new_stability < 0.1 {
            GradingAction::ReviewSoon
        } else {
            GradingAction::Continue
        };

        GradingResult {
            new_stability_days: new_stability,
            new_next_review_at: next_review,
            review_count: new_review_count,
            recommended_action: action,
        }
    }
}

impl Default for GradingEngine {
    fn default() -> Self {
        Self::new()
    }
}
