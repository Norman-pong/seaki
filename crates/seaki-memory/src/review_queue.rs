//! ReviewQueue: manages ReviewCards sorted by next review time.

use crate::review_card::ReviewCard;
use std::sync::RwLock;

#[derive(Debug)]
pub struct ReviewQueue {
    cards: RwLock<Vec<ReviewCard>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewQueueError {
    CardNotFound(String),
}

impl ReviewQueue {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cards: RwLock::new(Vec::new()),
        }
    }

    /// 添加卡片到队列。
    pub fn enqueue(&self, card: ReviewCard) {
        self.cards.write().expect("poisoned lock").push(card);
    }

    /// 获取所有到期的卡片（now >= next_review_at）。
    #[must_use]
    pub fn due_cards(&self, now: u64) -> Vec<ReviewCard> {
        let cards = self.cards.read().expect("poisoned lock");
        let mut result: Vec<ReviewCard> = cards
            .iter()
            .filter(|c| now >= c.next_review_at)
            .cloned()
            .collect();
        result.sort_by_key(|c| c.next_review_at);
        result
    }

    /// 获取指定数量的到期卡片（按 next_review_at 排序）。
    #[must_use]
    pub fn next_due(&self, now: u64, limit: usize) -> Vec<ReviewCard> {
        let mut result = self.due_cards(now);
        result.truncate(limit);
        result
    }

    /// 按 retention 排序获取即将到期的卡片（用于预览）。
    ///
    /// 返回 `next_review_at` 在 `within_hours` 小时内的卡片，
    /// 按 `next_review_at` 升序排列。
    #[must_use]
    pub fn upcoming(&self, now: u64, within_hours: u64) -> Vec<ReviewCard> {
        let window = within_hours * 3600;
        let cards = self.cards.read().expect("poisoned lock");
        let mut result: Vec<ReviewCard> = cards
            .iter()
            .filter(|c| {
                let delta = c.next_review_at.saturating_sub(now);
                delta <= window && now < c.next_review_at
            })
            .cloned()
            .collect();
        result.sort_by_key(|c| c.next_review_at);
        result
    }

    /// 移除指定 card_id 的卡片。
    pub fn remove(&self, card_id: &str) -> Option<ReviewCard> {
        let mut cards = self.cards.write().expect("poisoned lock");
        cards
            .iter()
            .position(|c| c.card_id == card_id)
            .map(|pos| cards.remove(pos))
    }

    /// 更新卡片（用于 grading 后更新 stability 和 next_review_at）。
    ///
    /// # Errors
    ///
    /// 当队列中不存在指定 card_id 时返回 [`ReviewQueueError::CardNotFound`]。
    pub fn update_card(&self, card: ReviewCard) -> Result<(), ReviewQueueError> {
        let mut cards = self.cards.write().expect("poisoned lock");
        let pos = cards
            .iter()
            .position(|c| c.card_id == card.card_id)
            .ok_or_else(|| ReviewQueueError::CardNotFound(card.card_id.clone()))?;
        cards[pos] = card;
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.cards.read().expect("poisoned lock").len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cards.read().expect("poisoned lock").is_empty()
    }
}

impl Default for ReviewQueue {
    fn default() -> Self {
        Self::new()
    }
}
