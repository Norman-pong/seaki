//! MemoryCollector: stub extractor that proposes MemoryItem from session/wiki/approval.
//!
//! Current implementation uses simple keyword heuristics.
//! Full LLM-based extraction is deferred to M3.

use crate::memory_item::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, TrustLevel,
};
use seaki_index::IndexScope;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct MemoryCollector;

impl MemoryCollector {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 从会话摘要中提取包含偏好/约定关键词的句子。
    #[must_use]
    pub fn extract_from_session(
        &self,
        session_summary: &str,
        scope: &IndexScope,
        session_id: &str,
    ) -> Vec<MemoryItem> {
        let keywords = ["prefer", "convention", "always", "never", "must", "should"];
        let sentences: Vec<&str> = session_summary
            .split(['.', '!', '?'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        sentences
            .into_iter()
            .filter(|s| {
                let lower = s.to_lowercase();
                keywords.iter().any(|kw| lower.contains(*kw))
            })
            .map(|s| {
                let content = if s.ends_with('.') {
                    s.to_string()
                } else {
                    format!("{}.", s)
                };
                build_item(
                    MemoryKind::UserPreference,
                    scope,
                    content,
                    MemoryOrigin::SessionHistory,
                    "session_summary",
                    Some(session_id.to_string()),
                    None,
                )
            })
            .collect()
    }

    /// 从 wiki patch 中提取新增的约束性语句（以 `+` 开头的行）。
    #[must_use]
    pub fn extract_from_wiki_patch(
        &self,
        patch_content: &str,
        scope: &IndexScope,
        patch_hash: &str,
    ) -> Vec<MemoryItem> {
        let constraint_keywords = ["must", "should", "always", "never", "require", "forbid"];

        patch_content
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++") && line.len() > 1)
            .map(|line| line.trim_start_matches('+').trim())
            .filter(|line| {
                let lower = line.to_lowercase();
                constraint_keywords.iter().any(|kw| lower.contains(*kw))
            })
            .map(|content| {
                build_item(
                    MemoryKind::ProjectConvention,
                    scope,
                    content.to_string(),
                    MemoryOrigin::WikiPatch,
                    "wiki_patch",
                    None,
                    Some(patch_hash.to_string()),
                )
            })
            .collect()
    }

    /// 从审批决策中提取安全规则或约定。
    #[must_use]
    pub fn extract_from_approval(
        &self,
        decision_summary: &str,
        scope: &IndexScope,
        _actor_id: &str,
    ) -> Vec<MemoryItem> {
        let safety_keywords = [
            "security",
            "safety",
            "unsafe",
            "risk",
            "vulnerable",
            "protect",
        ];
        let sentences: Vec<&str> = decision_summary
            .split(['.', '!', '?'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        sentences
            .into_iter()
            .filter(|s| {
                let lower = s.to_lowercase();
                safety_keywords.iter().any(|kw| lower.contains(*kw))
            })
            .map(|s| {
                let content = if s.ends_with('.') {
                    s.to_string()
                } else {
                    format!("{}.", s)
                };
                build_item(
                    MemoryKind::SafetyRule,
                    scope,
                    content,
                    MemoryOrigin::ApprovalDecision,
                    "approval_decision",
                    None,
                    None,
                )
            })
            .collect()
    }
}

impl Default for MemoryCollector {
    fn default() -> Self {
        Self::new()
    }
}

fn next_id() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn build_item(
    kind: MemoryKind,
    scope: &IndexScope,
    content: String,
    origin: MemoryOrigin,
    extraction_method: &str,
    session_id: Option<String>,
    wiki_patch_hash: Option<String>,
) -> MemoryItem {
    let now = current_timestamp();
    MemoryItem {
        memory_id: format!("mem-{}", next_id()),
        kind,
        scope: scope.clone(),
        content,
        source_citation: None,
        proposed_at: now,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: Some(now.saturating_add(30 * 24 * 60 * 60)),
        status: MemoryStatus::Proposed,
        trust_level: TrustLevel::Unverified,
        confirmed_by: None,
        provenance: MemoryProvenance {
            origin,
            extraction_method: extraction_method.to_string(),
            session_id,
            wiki_patch_hash,
        },
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
