use std::path::Path;
use std::time::SystemTime;

use crate::audit::AuditRecord;
use crate::grant::{CapabilityStore, GenericUseCapabilityRequest, UseCapabilityRequest};
use crate::hash_text;
use crate::path::WorkspacePathPolicy;
use crate::types::FILE_READ_CAPABILITY;
use crate::types::{PolicyDecision, PolicyEvaluation, PolicyReason, PolicyResult, SideEffectLevel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub approval_id: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub operation: String,
    pub reason: String,
    pub status: crate::types::ApprovalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecision {
    pub approval_id: String,
    pub policy_decision_id: String,
    pub scope_hash: String,
    pub decided_by: String,
    pub status: crate::types::ApprovalStatus,
    pub decided_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReadPolicyRequest {
    pub actor_id: String,
    pub workspace_id: String,
    pub audience: String,
    pub operation: String,
    pub path: std::path::PathBuf,
    pub capability_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityPolicyRequest {
    pub actor_id: String,
    pub workspace_id: String,
    pub capability: String,
    pub operation: String,
    pub capability_id: Option<String>,
    pub side_effect_level: SideEffectLevel,
    pub audience: String,
}

#[derive(Debug)]
pub struct PolicyEngine {
    workspace_policy: WorkspacePathPolicy,
    capability_store: CapabilityStore,
    fixed_now: Option<SystemTime>,
}

impl PolicyEngine {
    #[must_use]
    pub fn new(workspace_policy: WorkspacePathPolicy) -> Self {
        Self {
            workspace_policy,
            capability_store: CapabilityStore::new(),
            fixed_now: None,
        }
    }

    #[must_use]
    pub fn with_fixed_now(workspace_policy: WorkspacePathPolicy, now: SystemTime) -> Self {
        Self {
            workspace_policy,
            capability_store: CapabilityStore::new(),
            fixed_now: Some(now),
        }
    }

    pub fn with_capability_store(
        workspace_policy: WorkspacePathPolicy,
        capability_store: CapabilityStore,
    ) -> Self {
        Self {
            workspace_policy,
            capability_store,
            fixed_now: None,
        }
    }

    pub fn with_capability_store_and_fixed_now(
        workspace_policy: WorkspacePathPolicy,
        capability_store: CapabilityStore,
        now: SystemTime,
    ) -> Self {
        Self {
            workspace_policy,
            capability_store,
            fixed_now: Some(now),
        }
    }

    pub fn capability_store(&self) -> &CapabilityStore {
        &self.capability_store
    }

    fn now(&self) -> SystemTime {
        self.fixed_now.unwrap_or_else(SystemTime::now)
    }

    /// 根据工作区路径策略和能力授权评估文件读取请求。
    ///
    /// # Errors
    ///
    /// 当路径 canonicalize 失败或能力存储操作失败时返回错误。
    pub fn authorize_file_read(
        &self,
        request: &FileReadPolicyRequest,
    ) -> PolicyResult<PolicyEvaluation> {
        let now = self.now();
        let workspace_decision = self
            .workspace_policy
            .classify_workspace_read(&request.path)?;
        if workspace_decision.decision.permits_side_effect() {
            return Ok(PolicyEvaluation::allow(
                PolicyReason::WorkspaceAllowlist,
                AuditRecord::policy_decision(
                    request,
                    &workspace_decision.canonical_path,
                    now,
                    PolicyDecision::Allow,
                    PolicyReason::WorkspaceAllowlist,
                ),
            ));
        }
        if workspace_decision.reason == PolicyReason::PathDenied {
            return Ok(PolicyEvaluation::deny(
                PolicyReason::PathDenied,
                AuditRecord::policy_decision(
                    request,
                    &workspace_decision.canonical_path,
                    now,
                    PolicyDecision::Deny,
                    PolicyReason::PathDenied,
                ),
            ));
        }

        let Some(capability_id) = request.capability_id.as_ref() else {
            return Ok(PolicyEvaluation::deny(
                workspace_decision.reason.clone(),
                AuditRecord::policy_decision(
                    request,
                    &workspace_decision.canonical_path,
                    now,
                    PolicyDecision::Deny,
                    workspace_decision.reason,
                ),
            ));
        };

        let use_request = UseCapabilityRequest {
            capability_id: capability_id.clone(),
            subject_actor_id: request.actor_id.clone(),
            audience: request.audience.clone(),
            workspace_id: request.workspace_id.clone(),
            capability: FILE_READ_CAPABILITY.to_string(),
            operation: request.operation.clone(),
            canonical_path: workspace_decision.canonical_path.clone(),
            now,
        };

        match self
            .capability_store
            .consume_file_read_grant(&use_request)?
        {
            Ok(consumption) => Ok(PolicyEvaluation::allow(
                PolicyReason::CapabilityGrant,
                AuditRecord::capability_consumed(
                    request,
                    &workspace_decision.canonical_path,
                    capability_id,
                    Some(consumption.grant_fingerprint),
                    now,
                    PolicyDecision::Allow,
                    PolicyReason::CapabilityGrant,
                ),
            )),
            Err(failure) => {
                let reason = PolicyReason::CapabilityGrantRejected(failure.rejection);
                Ok(PolicyEvaluation::deny(
                    reason.clone(),
                    AuditRecord::capability_consumed(
                        request,
                        &workspace_decision.canonical_path,
                        capability_id,
                        failure.grant_fingerprint,
                        now,
                        PolicyDecision::Deny,
                        reason,
                    ),
                ))
            }
        }
    }

    /// 根据能力授权和副作用级别评估通用能力请求。
    ///
    /// # Errors
    ///
    /// 当能力存储操作失败时返回错误。
    pub fn authorize_capability(
        &self,
        request: &CapabilityPolicyRequest,
    ) -> PolicyResult<PolicyEvaluation> {
        let now = self.now();

        if request.side_effect_level == SideEffectLevel::None {
            return Ok(PolicyEvaluation::allow(
                PolicyReason::WorkspaceAllowlist,
                AuditRecord::generic_policy_decision(
                    request,
                    now,
                    PolicyDecision::Allow,
                    PolicyReason::WorkspaceAllowlist,
                ),
            ));
        }

        let has_grant = if let Some(ref capability_id) = request.capability_id {
            let use_request = GenericUseCapabilityRequest {
                capability_id: capability_id.clone(),
                subject_actor_id: request.actor_id.clone(),
                audience: request.audience.clone(),
                workspace_id: request.workspace_id.clone(),
                capability: request.capability.clone(),
                operation: request.operation.clone(),
                now,
            };
            self.capability_store
                .consume_generic_grant(&use_request)?
                .is_ok()
        } else {
            self.capability_store.has_valid_generic_grant(
                &request.actor_id,
                &request.workspace_id,
                &request.capability,
                &request.operation,
                &request.audience,
                now,
            )?
        };

        if has_grant {
            Ok(PolicyEvaluation::allow(
                PolicyReason::CapabilityGrant,
                AuditRecord::generic_policy_decision(
                    request,
                    now,
                    PolicyDecision::Allow,
                    PolicyReason::CapabilityGrant,
                ),
            ))
        } else {
            Ok(PolicyEvaluation {
                decision: PolicyDecision::RequireApproval,
                reason: PolicyReason::MissingCapabilityGrant,
                audit: AuditRecord::generic_policy_decision(
                    request,
                    now,
                    PolicyDecision::RequireApproval,
                    PolicyReason::MissingCapabilityGrant,
                ),
            })
        }
    }
}

pub(crate) fn policy_decision_id(
    request: &FileReadPolicyRequest,
    canonical_path: &Path,
    occurred_at: SystemTime,
) -> String {
    hash_text(&format!(
        "{}\n{}\n{}\n{}\n{}\n{:?}",
        request.actor_id,
        request.workspace_id,
        request.audience,
        request.operation,
        canonical_path.display(),
        occurred_at
    ))
}
