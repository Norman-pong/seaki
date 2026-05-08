//! CardGenerator: 从各种来源生成 ReviewCard 的启发式 stub 生成器。
//!
//! LLM 驱动的智能提取将在 M3 中实现，当前使用规则化启发式。

use crate::memory_item::{MemoryItem, MemoryStatus};
use crate::memory_store::MemoryStore;
use crate::review_card::{CardDifficulty, ReviewCard};
use seaki_index::IndexScope;

/// 从各种来源生成 ReviewCard 的生成器。
#[derive(Debug, Clone, Copy, Default)]
pub struct CardGenerator;

impl CardGenerator {
    /// 创建新的 CardGenerator。
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 从单个 MemoryItem 生成 ReviewCard（仅 Active 状态）。
    ///
    /// - 内容长度 < 10 或 > 512 时跳过
    /// - `SafetyRule` 和 `ProjectConvention` 类型优先生成（过滤条件已体现在 difficulty 映射中）
    #[must_use]
    pub fn from_memory_item(&self, item: &MemoryItem, now: u64) -> Option<ReviewCard> {
        if item.status != MemoryStatus::Active {
            return None;
        }
        if item.content.len() < 10 || item.content.len() > 512 {
            return None;
        }
        Some(ReviewCard::from_memory_item(item, now))
    }

    /// 批量从 MemoryStore 中所有 Active items 生成卡片。
    #[must_use]
    pub fn generate_from_store(&self, store: &MemoryStore, now: u64) -> Vec<ReviewCard> {
        store
            .items_by_status(MemoryStatus::Active)
            .into_iter()
            .filter_map(|item| self.from_memory_item(item, now))
            .collect()
    }

    /// 从 wiki/source 文本生成 ReviewCard（stub）。
    ///
    /// 按句号分句，每段生成一个卡片：
    /// - question = 第一句（不超过 120 字符）
    /// - answer = 完整段落
    /// - difficulty = Medium（默认）
    /// - source = title
    #[must_use]
    pub fn from_wiki_text(
        &self,
        title: &str,
        body: &str,
        scope: &IndexScope,
        now: u64,
    ) -> Vec<ReviewCard> {
        let mut cards = Vec::new();
        for paragraph in body.split('\n').map(str::trim).filter(|s| !s.is_empty()) {
            let sentences: Vec<&str> = paragraph
                .split(['。', '.', '！', '!', '？', '?'])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if sentences.is_empty() {
                continue;
            }
            let question = truncate_to_char_boundaries(sentences[0], 120);
            let answer = paragraph.to_string();
            if question.len() < 10 || answer.len() > 512 {
                continue;
            }
            let card = ReviewCard {
                card_id: format!("card_wiki_{}_{}", now, cards.len()),
                memory_id: None,
                scope: scope.clone(),
                question,
                answer,
                source: Some(title.to_string()),
                created_at: now,
                last_reviewed_at: None,
                stability_days: CardDifficulty::Medium.initial_stability(),
                retention_threshold: CardDifficulty::Medium.default_threshold(),
                next_review_at: now
                    .saturating_add((CardDifficulty::Medium.initial_stability() * 86400.0) as u64),
                review_count: 0,
                difficulty: CardDifficulty::Medium,
            };
            cards.push(card);
        }
        cards
    }

    /// 从会话摘要生成 ReviewCard（stub）。
    ///
    /// 提取包含关键动作词的句子生成卡片。
    /// 关键词：must, should, never, always, remember, note 及其常见中文对应。
    #[must_use]
    pub fn from_session_summary(
        &self,
        summary: &str,
        scope: &IndexScope,
        session_id: &str,
        now: u64,
    ) -> Vec<ReviewCard> {
        let keywords = [
            "must", "should", "never", "always", "remember", "note", "必须", "应该", "绝不",
            "总是", "记得", "注意",
        ];
        let mut cards = Vec::new();
        for sentence in summary
            .split(['。', '.', '！', '!', '？', '?', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let lower = sentence.to_lowercase();
            if !keywords.iter().any(|kw| lower.contains(kw)) {
                continue;
            }
            if sentence.len() < 10 || sentence.len() > 512 {
                continue;
            }
            let card = ReviewCard {
                card_id: format!("card_session_{}_{}", session_id, cards.len()),
                memory_id: None,
                scope: scope.clone(),
                question: sentence.to_string(),
                answer: sentence.to_string(),
                source: Some(format!("session:{session_id}")),
                created_at: now,
                last_reviewed_at: None,
                stability_days: CardDifficulty::Medium.initial_stability(),
                retention_threshold: CardDifficulty::Medium.default_threshold(),
                next_review_at: now
                    .saturating_add((CardDifficulty::Medium.initial_stability() * 86400.0) as u64),
                review_count: 0,
                difficulty: CardDifficulty::Medium,
            };
            cards.push(card);
        }
        cards
    }
}

fn truncate_to_char_boundaries(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}
