//! TopicClusterer: 基于关键词重叠的简化 topic 聚类（stub）。
//!
//! 真实 NLP 聚类算法将在后续阶段实现。

use crate::memory_item::MemoryItem;
use crate::review_card::ReviewCard;
use std::collections::HashSet;

/// 表示一个聚类主题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub topic_id: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub memory_ids: Vec<String>,
    pub card_ids: Vec<String>,
    pub created_at: u64,
}

/// 简化的关键词重叠聚类器。
#[derive(Debug, Clone, Copy, Default)]
pub struct TopicClusterer;

impl TopicClusterer {
    /// 创建新的 TopicClusterer。
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 从 MemoryItem 列表中聚类出 topics。
    ///
    /// 算法：
    /// 1. 提取每个 item 的关键词
    /// 2. 计算 item 之间的关键词 Jaccard 相似度
    /// 3. 相似度 > threshold 的归为同一 topic
    /// 4. 为每个 topic 生成名称（出现频率最高的 2-3 个关键词组合）
    pub fn cluster_memory_items(
        &self,
        items: &[&MemoryItem],
        threshold: f64,
        now: u64,
    ) -> Vec<Topic> {
        let keywords_list: Vec<(String, Vec<String>)> = items
            .iter()
            .map(|item| (item.memory_id.clone(), extract_keywords(&item.content)))
            .collect();

        let mut assigned = vec![false; keywords_list.len()];
        let mut topics: Vec<Topic> = Vec::new();
        let mut topic_counter = 0usize;

        for i in 0..keywords_list.len() {
            if assigned[i] {
                continue;
            }
            let mut cluster_indices = vec![i];
            assigned[i] = true;

            for j in (i + 1)..keywords_list.len() {
                if assigned[j] {
                    continue;
                }
                let sim = jaccard_similarity(&keywords_list[i].1, &keywords_list[j].1);
                if sim > threshold {
                    cluster_indices.push(j);
                    assigned[j] = true;
                }
            }

            let cluster_keywords: Vec<String> = cluster_indices
                .iter()
                .flat_map(|idx| keywords_list[*idx].1.clone())
                .collect();

            let name = generate_topic_name(&cluster_keywords, &keywords_list, &cluster_indices);
            let memory_ids: Vec<String> = cluster_indices
                .iter()
                .map(|idx| keywords_list[*idx].0.clone())
                .collect();

            topics.push(Topic {
                topic_id: format!("topic_{topic_counter}"),
                name,
                keywords: top_keywords(&cluster_keywords, 5),
                memory_ids,
                card_ids: Vec::new(),
                created_at: now,
            });
            topic_counter += 1;
        }

        topics
    }

    /// 从 ReviewCard 列表中聚类出 topics。
    pub fn cluster_cards(&self, cards: &[&ReviewCard], threshold: f64, now: u64) -> Vec<Topic> {
        let keywords_list: Vec<(String, Vec<String>)> = cards
            .iter()
            .map(|card| {
                let content = format!("{} {}", card.question, card.answer);
                (card.card_id.clone(), extract_keywords(&content))
            })
            .collect();

        let mut assigned = vec![false; keywords_list.len()];
        let mut topics: Vec<Topic> = Vec::new();
        let mut topic_counter = 0usize;

        for i in 0..keywords_list.len() {
            if assigned[i] {
                continue;
            }
            let mut cluster_indices = vec![i];
            assigned[i] = true;

            for j in (i + 1)..keywords_list.len() {
                if assigned[j] {
                    continue;
                }
                let sim = jaccard_similarity(&keywords_list[i].1, &keywords_list[j].1);
                if sim > threshold {
                    cluster_indices.push(j);
                    assigned[j] = true;
                }
            }

            let cluster_keywords: Vec<String> = cluster_indices
                .iter()
                .flat_map(|idx| keywords_list[*idx].1.clone())
                .collect();

            let name = generate_topic_name(&cluster_keywords, &keywords_list, &cluster_indices);
            let card_ids: Vec<String> = cluster_indices
                .iter()
                .map(|idx| keywords_list[*idx].0.clone())
                .collect();

            topics.push(Topic {
                topic_id: format!("topic_{topic_counter}"),
                name,
                keywords: top_keywords(&cluster_keywords, 5),
                memory_ids: Vec::new(),
                card_ids,
                created_at: now,
            });
            topic_counter += 1;
        }

        topics
    }

    /// 将新 item 的关键词分配到最匹配的 topic（或创建新 topic）。
    ///
    /// 返回分配到的 topic_id。如果没有匹配且无法创建新 topic（需要外部计数器），
    /// 则返回一个基于时间的占位 id。此处 stub 实现使用固定命名。
    pub fn assign_to_topic(
        &self,
        item_keywords: &[String],
        topics: &mut Vec<Topic>,
        threshold: f64,
        now: u64,
    ) -> String {
        let mut best_topic_id = None;
        let mut best_sim = 0.0;

        for topic in &*topics {
            let sim = jaccard_similarity(item_keywords, &topic.keywords);
            if sim > threshold && sim > best_sim {
                best_sim = sim;
                best_topic_id = Some(topic.topic_id.clone());
            }
        }

        if let Some(topic_id) = best_topic_id {
            topic_id
        } else {
            let new_topic_id = format!("topic_auto_{now}");
            let name = if item_keywords.len() >= 2 {
                format!("{} {}", item_keywords[0], item_keywords[1])
            } else if item_keywords.len() == 1 {
                item_keywords[0].clone()
            } else {
                "misc".to_string()
            };
            topics.push(Topic {
                topic_id: new_topic_id.clone(),
                name,
                keywords: item_keywords.to_vec(),
                memory_ids: Vec::new(),
                card_ids: Vec::new(),
                created_at: now,
            });
            new_topic_id
        }
    }
}

/// 停用词列表（简化中文 + 英文）。
pub const STOP_WORDS: &[&str] = &[
    // English
    "the",
    "a",
    "an",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "have",
    "has",
    "had",
    "do",
    "does",
    "did",
    "will",
    "would",
    "could",
    "should",
    "may",
    "might",
    "must",
    "shall",
    "can",
    "need",
    "dare",
    "ought",
    "used",
    "to",
    "of",
    "in",
    "for",
    "on",
    "with",
    "at",
    "by",
    "from",
    "as",
    "into",
    "through",
    "during",
    "before",
    "after",
    "above",
    "below",
    "between",
    "under",
    "and",
    "but",
    "or",
    "yet",
    "so",
    "if",
    "because",
    "although",
    "though",
    "while",
    "where",
    "when",
    "that",
    "which",
    "who",
    "whom",
    "whose",
    "what",
    "this",
    "these",
    "those",
    "i",
    "you",
    "he",
    "she",
    "it",
    "we",
    "they",
    "me",
    "him",
    "her",
    "us",
    "them",
    "my",
    "your",
    "his",
    "its",
    "our",
    "their",
    "mine",
    "yours",
    "hers",
    "ours",
    "theirs",
    "myself",
    "yourself",
    "himself",
    "herself",
    "itself",
    "ourselves",
    "yourselves",
    "themselves",
    // Chinese
    "的",
    "了",
    "在",
    "是",
    "我",
    "有",
    "和",
    "就",
    "不",
    "人",
    "都",
    "一",
    "一个",
    "上",
    "也",
    "很",
    "到",
    "说",
    "要",
    "去",
    "你",
    "会",
    "着",
    "没有",
    "看",
    "好",
    "自己",
    "这",
];

fn extract_keywords(content: &str) -> Vec<String> {
    content
        .split(|c: char| !(c.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(&c)))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .filter(|s| !STOP_WORDS.contains(&s.as_str()))
        .filter(|s| {
            s.chars().count() > 3 || s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    let set_a: HashSet<_> = a.iter().cloned().collect();
    let set_b: HashSet<_> = b.iter().cloned().collect();
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn top_keywords(keywords: &[String], n: usize) -> Vec<String> {
    let mut freq = std::collections::HashMap::new();
    for kw in keywords {
        *freq.entry(kw.clone()).or_insert(0) += 1;
    }
    let mut pairs: Vec<_> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs.into_iter().take(n).map(|(kw, _)| kw).collect()
}

fn generate_topic_name(
    _cluster_keywords: &[String],
    keywords_list: &[(String, Vec<String>)],
    cluster_indices: &[usize],
) -> String {
    let mut freq = std::collections::HashMap::new();
    for idx in cluster_indices {
        for kw in &keywords_list[*idx].1 {
            *freq.entry(kw.clone()).or_insert(0) += 1;
        }
    }
    let mut pairs: Vec<_> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let top: Vec<String> = pairs.into_iter().take(3).map(|(kw, _)| kw).collect();
    if top.is_empty() {
        "untitled".to_string()
    } else {
        top.join(" ")
    }
}
