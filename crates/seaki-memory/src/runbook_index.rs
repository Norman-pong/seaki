//! RunbookIndex: 按 topic 组织的可执行操作手册索引。

use crate::memory_item::{MemoryItem, MemoryKind};
use std::collections::HashMap;

/// 可执行的操作手册条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookEntry {
    pub entry_id: String,
    pub topic_id: String,
    pub title: String,
    pub description: String,
    pub pipeline_template_id: Option<String>,
    pub required_capabilities: Vec<String>,
    pub source_memory_ids: Vec<String>,
    pub created_at: u64,
}

/// Runbook 索引：按 topic 组织可执行手册。
#[derive(Debug)]
pub struct RunbookIndex {
    entries: HashMap<String, RunbookEntry>,
    topic_index: HashMap<String, Vec<String>>,
    next_id: usize,
}

impl RunbookIndex {
    /// 创建空的 RunbookIndex。
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            topic_index: HashMap::new(),
            next_id: 0,
        }
    }

    /// 添加一个 runbook entry。
    pub fn insert(&mut self, entry: RunbookEntry) {
        self.topic_index
            .entry(entry.topic_id.clone())
            .or_default()
            .push(entry.entry_id.clone());
        self.entries.insert(entry.entry_id.clone(), entry);
    }

    /// 按 topic_id 查找 entries。
    #[must_use]
    pub fn by_topic(&self, topic_id: &str) -> Vec<&RunbookEntry> {
        self.topic_index
            .get(topic_id)
            .map(|ids| ids.iter().filter_map(|id| self.entries.get(id)).collect())
            .unwrap_or_default()
    }

    /// 按关键词搜索 entries（标题和描述匹配，大小写不敏感）。
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&RunbookEntry> {
        let lower_query = query.to_lowercase();
        self.entries
            .values()
            .filter(|entry| {
                entry.title.to_lowercase().contains(&lower_query)
                    || entry.description.to_lowercase().contains(&lower_query)
            })
            .collect()
    }

    /// 按 capability 过滤 entries。
    #[must_use]
    pub fn by_capability(&self, capability: &str) -> Vec<&RunbookEntry> {
        self.entries
            .values()
            .filter(|entry| entry.required_capabilities.iter().any(|c| c == capability))
            .collect()
    }

    /// 从 MemoryItem 列表自动生成 runbook entries（stub）。
    ///
    /// 当前简化：`SafetyRule` 和 `WorkflowPattern` 类型的 item 自动转为 runbook。
    pub fn auto_generate(&mut self, items: &[&MemoryItem], now: u64) {
        for item in items {
            if !matches!(
                item.kind,
                MemoryKind::SafetyRule | MemoryKind::WorkflowPattern
            ) {
                continue;
            }
            let entry_id = format!("runbook_{}", self.next_id);
            self.next_id += 1;

            let title = if item.content.len() > 60 {
                format!("{}...", &item.content[..60])
            } else {
                item.content.clone()
            };

            let entry = RunbookEntry {
                entry_id,
                topic_id: format!("topic_{}", item.kind.as_str()),
                title,
                description: item.content.clone(),
                pipeline_template_id: None,
                required_capabilities: Vec::new(),
                source_memory_ids: vec![item.memory_id.clone()],
                created_at: now,
            };
            self.insert(entry);
        }
    }

    /// 返回 entries 数量。
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 检查索引是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for RunbookIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryKind {
    fn as_str(&self) -> &'static str {
        match self {
            MemoryKind::UserPreference => "user_preference",
            MemoryKind::ProjectConvention => "project_convention",
            MemoryKind::WorkflowPattern => "workflow_pattern",
            MemoryKind::SafetyRule => "safety_rule",
            MemoryKind::DerivedFact => "derived_fact",
        }
    }
}
