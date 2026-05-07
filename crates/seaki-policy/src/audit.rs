use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::engine::{policy_decision_id, CapabilityPolicyRequest, FileReadPolicyRequest};
use crate::grant::{generic_grant_fingerprint, CapabilityGrant, GenericCapabilityGrant};
use crate::hash_text;
use crate::types::{PolicyDecision, PolicyReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditAction {
    GrantIssued,
    PolicyDecision,
    CapabilityConsumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub policy_decision_id: String,
    pub action: AuditAction,
    pub occurred_at: SystemTime,
    pub actor_id: String,
    pub workspace_id: String,
    pub audience: String,
    pub operation: String,
    pub canonical_path: PathBuf,
    pub capability_id: Option<String>,
    pub grant_fingerprint: Option<String>,
    pub decision: PolicyDecision,
    pub reason: PolicyReason,
}

impl AuditRecord {
    pub(crate) fn grant_issued(grant: &CapabilityGrant) -> Self {
        Self {
            policy_decision_id: grant.policy_decision_id.clone(),
            action: AuditAction::GrantIssued,
            occurred_at: grant.not_before,
            actor_id: grant.subject_actor_id.clone(),
            workspace_id: grant.workspace_id.clone(),
            audience: grant.audience.clone(),
            operation: grant.operation.clone(),
            canonical_path: grant.canonical_path.clone(),
            capability_id: Some(grant.capability_id.clone()),
            grant_fingerprint: Some(grant.fingerprint()),
            decision: PolicyDecision::Allow,
            reason: PolicyReason::CapabilityGrant,
        }
    }

    pub(crate) fn policy_decision(
        request: &FileReadPolicyRequest,
        canonical_path: &Path,
        occurred_at: SystemTime,
        decision: PolicyDecision,
        reason: PolicyReason,
    ) -> Self {
        Self {
            policy_decision_id: policy_decision_id(request, canonical_path, occurred_at),
            action: AuditAction::PolicyDecision,
            occurred_at,
            actor_id: request.actor_id.clone(),
            workspace_id: request.workspace_id.clone(),
            audience: request.audience.clone(),
            operation: request.operation.clone(),
            canonical_path: canonical_path.to_path_buf(),
            capability_id: None,
            grant_fingerprint: None,
            decision,
            reason,
        }
    }

    pub(crate) fn capability_consumed(
        request: &FileReadPolicyRequest,
        canonical_path: &Path,
        capability_id: &str,
        grant_fingerprint: Option<String>,
        occurred_at: SystemTime,
        decision: PolicyDecision,
        reason: PolicyReason,
    ) -> Self {
        Self {
            policy_decision_id: policy_decision_id(request, canonical_path, occurred_at),
            action: AuditAction::CapabilityConsumed,
            occurred_at,
            actor_id: request.actor_id.clone(),
            workspace_id: request.workspace_id.clone(),
            audience: request.audience.clone(),
            operation: request.operation.clone(),
            canonical_path: canonical_path.to_path_buf(),
            capability_id: Some(capability_id.to_string()),
            grant_fingerprint,
            decision,
            reason,
        }
    }

    pub(crate) fn generic_grant_issued(grant: &GenericCapabilityGrant) -> Self {
        Self {
            policy_decision_id: grant.policy_decision_id.clone(),
            action: AuditAction::GrantIssued,
            occurred_at: grant.not_before,
            actor_id: grant.subject_actor_id.clone(),
            workspace_id: grant.workspace_id.clone(),
            audience: grant.audience.clone(),
            operation: grant.operation.clone(),
            canonical_path: PathBuf::new(),
            capability_id: Some(grant.capability_id.clone()),
            grant_fingerprint: Some(generic_grant_fingerprint(grant)),
            decision: PolicyDecision::Allow,
            reason: PolicyReason::CapabilityGrant,
        }
    }

    pub(crate) fn generic_policy_decision(
        request: &CapabilityPolicyRequest,
        occurred_at: SystemTime,
        decision: PolicyDecision,
        reason: PolicyReason,
    ) -> Self {
        Self {
            policy_decision_id: hash_text(&format!(
                "generic\n{}\n{}\n{}\n{}\n{:?}",
                request.actor_id,
                request.workspace_id,
                request.capability,
                request.operation,
                occurred_at
            )),
            action: AuditAction::PolicyDecision,
            occurred_at,
            actor_id: request.actor_id.clone(),
            workspace_id: request.workspace_id.clone(),
            audience: request.audience.clone(),
            operation: request.operation.clone(),
            canonical_path: PathBuf::new(),
            capability_id: request.capability_id.clone(),
            grant_fingerprint: None,
            decision,
            reason,
        }
    }
}
