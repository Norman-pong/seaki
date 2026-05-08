//! MemoryProposePipeline: policy check → injection scan → duplicate detection → scope binding → audit

use crate::{MemoryItem, MemoryStatus, MemoryStore, TrustLevel};
use seaki_index::IndexScope;
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::sync::Mutex;

pub struct MemoryProposePipeline {
    expected_scope: Option<IndexScope>,
    audit_log: Mutex<Vec<MemoryProposeAuditRecord>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProposeAuditRecord {
    pub actor: String,
    pub timestamp: u64,
    pub memory_id: String,
    pub proposed_content_hash: String,
}

impl MemoryProposePipeline {
    #[must_use]
    pub fn new() -> Self {
        Self {
            expected_scope: None,
            audit_log: Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn with_scope(expected_scope: IndexScope) -> Self {
        Self {
            expected_scope: Some(expected_scope),
            audit_log: Mutex::new(Vec::new()),
        }
    }

    /// 完整 propose 管道，返回处理后的 MemoryItem 状态。
    pub fn process(
        &self,
        item: MemoryItem,
        store: &MemoryStore,
        policy_check: &dyn MemoryPolicyChecker,
        now: u64,
    ) -> Result<MemoryItem, ProposePipelineError> {
        // 1. Policy check
        policy_check
            .check(&item)
            .map_err(ProposePipelineError::PolicyDenied)?;

        // 2. Injection scan
        if let Some(pattern) = detect_injection(&item.content) {
            return Err(ProposePipelineError::InjectionDetected(pattern));
        }

        // 3. Duplicate detection
        if let Some(duplicate_id) = find_duplicate(store, &item) {
            return Err(ProposePipelineError::DuplicateDetected(duplicate_id));
        }

        // 4. Scope binding
        if let Some(ref expected) = self.expected_scope {
            if item.scope != *expected {
                return Err(ProposePipelineError::ScopeBindingFailed(format!(
                    "expected scope {}/{}, got {}/{}",
                    expected.workspace_id,
                    expected.account_id,
                    item.scope.workspace_id,
                    item.scope.account_id
                )));
            }
        }

        // 5. Audit
        let actor = item
            .confirmed_by
            .clone()
            .unwrap_or_else(|| format!("{:?}", item.provenance.origin));
        let proposed_content_hash = hash_content(&item.content);
        let record = MemoryProposeAuditRecord {
            actor: actor.clone(),
            timestamp: now,
            memory_id: item.memory_id.clone(),
            proposed_content_hash: proposed_content_hash.clone(),
        };

        {
            let mut log = self.audit_log.lock().expect("audit log mutex poisoned");
            log.push(record.clone());
        }

        tracing::info!(
            actor = %actor,
            timestamp = now,
            memory_id = %item.memory_id,
            proposed_content_hash = %proposed_content_hash,
            "memory_propose_audited"
        );

        Ok(item)
    }

    /// 取出当前累积的 audit log 并清空内部缓存。
    #[must_use]
    pub fn take_audit_log(&self) -> Vec<MemoryProposeAuditRecord> {
        let mut log = self.audit_log.lock().expect("audit log mutex poisoned");
        std::mem::take(&mut *log)
    }
}

impl Default for MemoryProposePipeline {
    fn default() -> Self {
        Self::new()
    }
}

pub trait MemoryPolicyChecker: Send + Sync {
    fn check(&self, item: &MemoryItem) -> Result<(), PolicyCheckError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposePipelineError {
    PolicyDenied(PolicyCheckError),
    InjectionDetected(String),
    DuplicateDetected(String),
    ScopeBindingFailed(String),
    InvalidContent(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyCheckError {
    ContentTooLong { max: usize, actual: usize },
    ForbiddenKeywords(Vec<String>),
    UntrustedSource,
    ScopeMismatch,
}

pub struct DefaultMemoryPolicyChecker {
    max_content_length: usize,
    forbidden_keywords: Vec<String>,
}

impl DefaultMemoryPolicyChecker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            max_content_length: 4096,
            forbidden_keywords: vec![
                "ignore previous instructions".to_string(),
                "system prompt".to_string(),
                "override constraints".to_string(),
                "DAN mode".to_string(),
            ],
        }
    }
}

impl Default for DefaultMemoryPolicyChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryPolicyChecker for DefaultMemoryPolicyChecker {
    fn check(&self, item: &MemoryItem) -> Result<(), PolicyCheckError> {
        if item.content.len() > self.max_content_length {
            return Err(PolicyCheckError::ContentTooLong {
                max: self.max_content_length,
                actual: item.content.len(),
            });
        }

        let lower = item.content.to_lowercase();
        let found: Vec<String> = self
            .forbidden_keywords
            .iter()
            .filter(|kw| lower.contains(&kw.to_lowercase()))
            .cloned()
            .collect();
        if !found.is_empty() {
            return Err(PolicyCheckError::ForbiddenKeywords(found));
        }

        if item.trust_level == TrustLevel::Unverified && item.source_citation.is_none() {
            return Err(PolicyCheckError::UntrustedSource);
        }

        Ok(())
    }
}

fn detect_injection(content: &str) -> Option<String> {
    let lower = content.to_lowercase();
    let patterns = ["ignore", "override", "system prompt", "DAN"];
    for pattern in &patterns {
        if lower.contains(pattern) {
            return Some(pattern.to_string());
        }
    }
    None
}

fn find_duplicate(store: &MemoryStore, item: &MemoryItem) -> Option<String> {
    for existing in store.items_by_status(MemoryStatus::Active) {
        if existing.memory_id != item.memory_id && existing.content == item.content {
            return Some(existing.memory_id.clone());
        }
    }
    None
}

fn hash_content(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
