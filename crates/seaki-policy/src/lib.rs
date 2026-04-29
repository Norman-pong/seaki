use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{self, Write};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

pub const CAPABILITY_GRANT_VISIBILITY: &str = "opaque-id-only";
pub const FILE_READ_CAPABILITY: &str = "file.read";
const DEFAULT_DENY_ROOT_NAMES: &[&str] = &[".git", ".seaki", ".codex", ".agents"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny,
    RequireApproval,
}

impl PolicyDecision {
    #[must_use]
    pub const fn permits_side_effect(self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyReason {
    WorkspaceAllowlist,
    CapabilityGrant,
    PathOutsideWorkspace,
    PathDenied,
    MissingCapabilityGrant,
    CapabilityGrantRejected(CapabilityGrantRejection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEvaluation {
    pub decision: PolicyDecision,
    pub reason: PolicyReason,
    pub audit: AuditRecord,
}

impl PolicyEvaluation {
    fn allow(reason: PolicyReason, audit: AuditRecord) -> Self {
        Self {
            decision: PolicyDecision::Allow,
            reason,
            audit,
        }
    }

    fn deny(reason: PolicyReason, audit: AuditRecord) -> Self {
        Self {
            decision: PolicyDecision::Deny,
            reason,
            audit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    PathCanonicalizeFailed { path: PathBuf, message: String },
    CapabilityStorePoisoned,
    DuplicateCapabilityId(String),
    UnsupportedCapability(String),
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathCanonicalizeFailed { path, message } => {
                write!(f, "failed to canonicalize {}: {message}", path.display())
            }
            Self::CapabilityStorePoisoned => write!(f, "capability store lock poisoned"),
            Self::DuplicateCapabilityId(capability_id) => {
                write!(f, "duplicate capability id: {capability_id}")
            }
            Self::UnsupportedCapability(capability) => {
                write!(f, "unsupported capability: {capability}")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

pub type PolicyResult<T> = Result<T, PolicyError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePathPolicy {
    workspace_root: PathBuf,
    allow_roots: Vec<PathBuf>,
    deny_roots: Vec<PathBuf>,
}

impl WorkspacePathPolicy {
    /// 创建新的工作区路径策略。
    ///
    /// # Errors
    ///
    /// 当工作区根目录无法 canonicalize 时返回错误。
    pub fn try_new(workspace_root: impl AsRef<Path>) -> PolicyResult<Self> {
        let workspace_root = canonicalize_existing(workspace_root.as_ref())?;
        Ok(Self {
            allow_roots: vec![workspace_root.clone()],
            deny_roots: Vec::new(),
            workspace_root,
        })
    }

    /// 设置额外的允许根目录。
    ///
    /// # Errors
    ///
    /// 当任一允许根目录无法 canonicalize 时返回错误。
    pub fn with_allow_roots(
        mut self,
        allow_roots: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> PolicyResult<Self> {
        self.allow_roots = canonicalize_roots(allow_roots)?;
        Ok(self)
    }

    /// 设置拒绝根目录。
    ///
    /// # Errors
    ///
    /// 当任一拒绝根目录无法 canonicalize 时返回错误。
    pub fn with_deny_roots(
        mut self,
        deny_roots: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> PolicyResult<Self> {
        self.deny_roots = canonicalize_roots(deny_roots)?;
        Ok(self)
    }

    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// 对给定路径进行 canonicalize。
    ///
    /// # Errors
    ///
    /// 当路径无法 canonicalize 时返回错误。
    pub fn canonicalize_path(&self, path: impl AsRef<Path>) -> PolicyResult<PathBuf> {
        canonicalize_existing(path.as_ref())
    }

    #[must_use]
    pub fn is_workspace_read_allowed(&self, canonical_path: &Path) -> bool {
        self.is_allowlisted(canonical_path) && !self.is_denied(canonical_path)
    }

    /// 判断对工作区内给定路径的读取请求应被允许还是拒绝。
    ///
    /// # Errors
    ///
    /// 当路径无法 canonicalize 时返回错误。
    pub fn classify_workspace_read(
        &self,
        path: impl AsRef<Path>,
    ) -> PolicyResult<WorkspacePathDecision> {
        let canonical_path = self.canonicalize_path(path)?;
        let allowed = self.is_allowlisted(&canonical_path);
        let denied = self.is_denied(&canonical_path);
        let decision = if allowed && !denied {
            PolicyDecision::Allow
        } else {
            PolicyDecision::Deny
        };
        let reason = if denied {
            PolicyReason::PathDenied
        } else if allowed {
            PolicyReason::WorkspaceAllowlist
        } else {
            PolicyReason::PathOutsideWorkspace
        };

        Ok(WorkspacePathDecision {
            canonical_path,
            decision,
            reason,
        })
    }

    fn is_allowlisted(&self, canonical_path: &Path) -> bool {
        self.allow_roots
            .iter()
            .any(|root| path_contains(root, canonical_path))
    }

    fn is_denied(&self, canonical_path: &Path) -> bool {
        self.deny_roots
            .iter()
            .any(|root| path_contains(root, canonical_path))
            || self.is_default_denied(canonical_path)
    }

    fn is_default_denied(&self, canonical_path: &Path) -> bool {
        canonical_path
            .strip_prefix(&self.workspace_root)
            .ok()
            .and_then(|relative_path| relative_path.components().next())
            .and_then(|component| match component {
                std::path::Component::Normal(name) => Some(name),
                _ => None,
            })
            .is_some_and(|name| {
                DEFAULT_DENY_ROOT_NAMES
                    .iter()
                    .any(|denied| name == OsStr::new(denied))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePathDecision {
    pub canonical_path: PathBuf,
    pub decision: PolicyDecision,
    pub reason: PolicyReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub approval_id: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub operation: String,
    pub reason: String,
    pub status: ApprovalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecision {
    pub approval_id: String,
    pub policy_decision_id: String,
    pub scope_hash: String,
    pub decided_by: String,
    pub status: ApprovalStatus,
    pub decided_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileResourceSnapshot {
    len: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrant {
    capability_id: String,
    subject_actor_id: String,
    workspace_id: String,
    capability: String,
    audience: String,
    operation: String,
    canonical_path: PathBuf,
    resource: FileResourceSnapshot,
    max_bytes: u64,
    declared_mime: Option<String>,
    not_before: SystemTime,
    expires_at: SystemTime,
    uses_remaining: u32,
    granted_by: String,
    approval_id: String,
    policy_decision_id: String,
    revoked_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrantHandle {
    pub capability_id: String,
    pub audit: AuditRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReadGrantInput {
    pub capability_id: String,
    pub subject_actor_id: String,
    pub workspace_id: String,
    pub audience: String,
    pub operation: String,
    pub path: PathBuf,
    pub max_bytes: u64,
    pub declared_mime: Option<String>,
    pub not_before: SystemTime,
    pub expires_at: SystemTime,
    pub granted_by: String,
    pub approval: ApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseCapabilityRequest {
    pub capability_id: String,
    pub subject_actor_id: String,
    pub audience: String,
    pub workspace_id: String,
    pub capability: String,
    pub operation: String,
    pub canonical_path: PathBuf,
    pub now: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityGrantRejection {
    Unknown,
    ApprovalNotApproved,
    WrongCapability,
    WrongAudience,
    WrongSubject,
    WrongWorkspace,
    WrongOperation,
    ScopeMismatch,
    ResourceChanged,
    ResourceTooLarge,
    ApprovalScopeMismatch,
    NotYetValid,
    Expired,
    Revoked,
    AlreadyUsed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub transaction_id: String,
    pub source_id: String,
    pub citation_ids: Vec<String>,
    pub thread_scope: String,
    pub audit_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelActionGrant {
    pub grant_id: String,
    pub scope: String,
    pub audience: String,
    pub ttl: Duration,
    pub uses_remaining: u32,
    pub idempotency_key: String,
    pub allowed_actions: Vec<String>,
    pub provenance: Provenance,
    pub expires_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueChannelActionGrantInput {
    pub grant_id: String,
    pub scope: String,
    pub audience: String,
    pub ttl: Duration,
    pub uses: u32,
    pub idempotency_key: String,
    pub allowed_actions: Vec<String>,
    pub provenance: Provenance,
    pub issued_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelActionGrantConsumption {
    pub grant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantError {
    DuplicateGrantId(String),
    GrantNotFound,
    GrantExpired,
    UsesExhausted,
}

impl fmt::Display for GrantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateGrantId(id) => write!(f, "duplicate grant id: {id}"),
            Self::GrantNotFound => write!(f, "grant not found"),
            Self::GrantExpired => write!(f, "grant expired"),
            Self::UsesExhausted => write!(f, "uses exhausted"),
        }
    }
}

impl std::error::Error for GrantError {}

#[derive(Debug, Default)]
pub struct CapabilityStore {
    grants: Mutex<HashMap<String, CapabilityGrant>>,
    channel_action_grants: Mutex<HashMap<String, ChannelActionGrant>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityConsumption {
    pub grant_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityUseFailure {
    pub rejection: CapabilityGrantRejection,
    pub grant_fingerprint: Option<String>,
}

impl CapabilityStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 签发文件读取能力授权。
    ///
    /// # Errors
    ///
    /// 当路径 canonicalize 失败、文件快照失败，或能力存储锁中毒/重复 ID/不支持的能力时返回错误。
    pub fn issue_file_read_grant(
        &self,
        input: FileReadGrantInput,
    ) -> PolicyResult<Result<CapabilityGrantHandle, CapabilityGrantRejection>> {
        if input.approval.status != ApprovalStatus::Approved {
            return Ok(Err(CapabilityGrantRejection::ApprovalNotApproved));
        }

        let canonical_path = canonicalize_existing(&input.path)?;
        let Some(resource) = snapshot_file(&canonical_path, input.max_bytes)? else {
            return Ok(Err(CapabilityGrantRejection::ResourceTooLarge));
        };
        let expected_scope_hash = file_read_grant_scope_hash(&FileReadGrantScope {
            subject_actor_id: &input.subject_actor_id,
            workspace_id: &input.workspace_id,
            audience: &input.audience,
            operation: &input.operation,
            canonical_path: &canonical_path,
            max_bytes: input.max_bytes,
            declared_mime: input.declared_mime.as_deref(),
            resource: &resource,
        });
        if input.approval.scope_hash != expected_scope_hash {
            return Ok(Err(CapabilityGrantRejection::ApprovalScopeMismatch));
        }
        let grant = CapabilityGrant {
            capability_id: input.capability_id,
            subject_actor_id: input.subject_actor_id,
            workspace_id: input.workspace_id,
            capability: FILE_READ_CAPABILITY.to_string(),
            audience: input.audience,
            operation: input.operation,
            canonical_path,
            resource,
            max_bytes: input.max_bytes,
            declared_mime: input.declared_mime,
            not_before: input.not_before,
            expires_at: input.expires_at,
            uses_remaining: 1,
            granted_by: input.granted_by,
            approval_id: input.approval.approval_id,
            policy_decision_id: input.approval.policy_decision_id,
            revoked_at: None,
        };
        let capability_id = grant.capability_id.clone();
        let audit = AuditRecord::grant_issued(&grant);

        self.insert(grant)?;

        Ok(Ok(CapabilityGrantHandle {
            capability_id,
            audit,
        }))
    }

    fn insert(&self, grant: CapabilityGrant) -> PolicyResult<()> {
        if grant.capability != FILE_READ_CAPABILITY {
            return Err(PolicyError::UnsupportedCapability(grant.capability));
        }
        if grant.uses_remaining != 1 {
            return Err(PolicyError::UnsupportedCapability(
                "file.read grants must be single-use".to_string(),
            ));
        }

        let mut grants = self
            .grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;

        if grants.contains_key(&grant.capability_id) {
            return Err(PolicyError::DuplicateCapabilityId(
                grant.capability_id.clone(),
            ));
        }

        grants.insert(grant.capability_id.clone(), grant);
        Ok(())
    }

    /// 消耗文件读取能力授权。
    ///
    /// # Errors
    ///
    /// 当能力存储锁中毒时返回错误。
    pub fn consume_file_read_grant(
        &self,
        request: &UseCapabilityRequest,
    ) -> PolicyResult<Result<CapabilityConsumption, CapabilityUseFailure>> {
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        let Some(grant) = grants.get_mut(&request.capability_id) else {
            return Ok(Err(CapabilityUseFailure {
                rejection: CapabilityGrantRejection::Unknown,
                grant_fingerprint: None,
            }));
        };
        let grant_fingerprint = grant.fingerprint();
        let reject = |rejection| {
            Ok(Err(CapabilityUseFailure {
                rejection,
                grant_fingerprint: Some(grant_fingerprint.clone()),
            }))
        };

        if grant.capability != request.capability || grant.capability != FILE_READ_CAPABILITY {
            return reject(CapabilityGrantRejection::WrongCapability);
        }
        if grant.audience != request.audience {
            return reject(CapabilityGrantRejection::WrongAudience);
        }
        if grant.subject_actor_id != request.subject_actor_id {
            return reject(CapabilityGrantRejection::WrongSubject);
        }
        if grant.workspace_id != request.workspace_id {
            return reject(CapabilityGrantRejection::WrongWorkspace);
        }
        if grant.operation != request.operation {
            return reject(CapabilityGrantRejection::WrongOperation);
        }
        if grant.canonical_path != request.canonical_path {
            return reject(CapabilityGrantRejection::ScopeMismatch);
        }
        if request.now < grant.not_before {
            return reject(CapabilityGrantRejection::NotYetValid);
        }
        if request.now >= grant.expires_at {
            return reject(CapabilityGrantRejection::Expired);
        }
        if grant.revoked_at.is_some() {
            return reject(CapabilityGrantRejection::Revoked);
        }
        if grant.uses_remaining == 0 {
            return reject(CapabilityGrantRejection::AlreadyUsed);
        }
        let Some(resource) = snapshot_file(&request.canonical_path, grant.max_bytes)? else {
            return reject(CapabilityGrantRejection::ResourceTooLarge);
        };
        if grant.resource != resource {
            return reject(CapabilityGrantRejection::ResourceChanged);
        }

        grant.uses_remaining -= 1;
        Ok(Ok(CapabilityConsumption { grant_fingerprint }))
    }

    /// 查询指定能力授权的剩余使用次数。
    ///
    /// # Errors
    ///
    /// 当能力存储锁中毒时返回错误。
    pub fn uses_remaining(&self, capability_id: &str) -> PolicyResult<Option<u32>> {
        let grants = self
            .grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        Ok(grants.get(capability_id).map(|grant| grant.uses_remaining))
    }

    /// 签发 channel 动作授权。
    ///
    /// # Errors
    ///
    /// 当 channel action 授权存储锁中毒时返回错误。
    pub fn issue_channel_action_grant(
        &self,
        input: IssueChannelActionGrantInput,
    ) -> PolicyResult<Result<ChannelActionGrant, GrantError>> {
        let mut grants = self
            .channel_action_grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        if grants.contains_key(&input.grant_id) {
            return Ok(Err(GrantError::DuplicateGrantId(input.grant_id)));
        }
        let grant = ChannelActionGrant {
            grant_id: input.grant_id.clone(),
            scope: input.scope,
            audience: input.audience,
            ttl: input.ttl,
            uses_remaining: input.uses,
            idempotency_key: input.idempotency_key,
            allowed_actions: input.allowed_actions,
            provenance: input.provenance,
            expires_at: input.issued_at + input.ttl,
        };
        grants.insert(input.grant_id, grant.clone());
        Ok(Ok(grant))
    }

    /// 消耗 channel 动作授权。
    ///
    /// # Errors
    ///
    /// 当 channel action 授权存储锁中毒时返回错误。
    pub fn consume_channel_action_grant(
        &self,
        grant_id: &str,
        now: SystemTime,
    ) -> PolicyResult<Result<ChannelActionGrantConsumption, GrantError>> {
        let mut grants = self
            .channel_action_grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        let Some(grant) = grants.get_mut(grant_id) else {
            return Ok(Err(GrantError::GrantNotFound));
        };
        if now >= grant.expires_at {
            return Ok(Err(GrantError::GrantExpired));
        }
        if grant.uses_remaining == 0 {
            return Ok(Err(GrantError::UsesExhausted));
        }
        grant.uses_remaining -= 1;
        Ok(Ok(ChannelActionGrantConsumption {
            grant_id: grant_id.to_string(),
        }))
    }

    /// 查询指定 channel 动作授权的剩余使用次数。
    ///
    /// # Errors
    ///
    /// 当 channel action 授权存储锁中毒时返回错误。
    pub fn channel_action_uses_remaining(&self, grant_id: &str) -> PolicyResult<Option<u32>> {
        let grants = self
            .channel_action_grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        Ok(grants.get(grant_id).map(|g| g.uses_remaining))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReadPolicyRequest {
    pub actor_id: String,
    pub workspace_id: String,
    pub audience: String,
    pub operation: String,
    pub path: PathBuf,
    pub capability_id: Option<String>,
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
}

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
    fn grant_issued(grant: &CapabilityGrant) -> Self {
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

    fn policy_decision(
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

    fn capability_consumed(
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
}

impl CapabilityGrant {
    fn fingerprint(&self) -> String {
        hash_text(&format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            self.capability_id,
            self.subject_actor_id,
            self.workspace_id,
            self.capability,
            self.audience,
            self.operation,
            self.canonical_path.display(),
            self.resource.len,
            self.resource.sha256,
            self.declared_mime.as_deref().unwrap_or(""),
            self.granted_by,
            self.approval_id,
            self.policy_decision_id
        ))
    }
}

fn canonicalize_existing(path: &Path) -> PolicyResult<PathBuf> {
    path.canonicalize()
        .map_err(|error| PolicyError::PathCanonicalizeFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn canonicalize_roots(
    roots: impl IntoIterator<Item = impl AsRef<Path>>,
) -> PolicyResult<Vec<PathBuf>> {
    roots
        .into_iter()
        .map(|root| canonicalize_existing(root.as_ref()))
        .collect()
}

fn path_contains(root: &Path, path: &Path) -> bool {
    path == root || path.starts_with(root)
}

fn snapshot_file(path: &Path, max_bytes: u64) -> PolicyResult<Option<FileResourceSnapshot>> {
    let mut file = File::open(path).map_err(|error| PolicyError::PathCanonicalizeFailed {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| PolicyError::PathCanonicalizeFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if bytes.len() as u64 > max_bytes {
        return Ok(None);
    }

    Ok(Some(FileResourceSnapshot {
        len: bytes.len() as u64,
        sha256: hash_bytes(&bytes),
    }))
}

struct FileReadGrantScope<'a> {
    subject_actor_id: &'a str,
    workspace_id: &'a str,
    audience: &'a str,
    operation: &'a str,
    canonical_path: &'a Path,
    max_bytes: u64,
    declared_mime: Option<&'a str>,
    resource: &'a FileResourceSnapshot,
}

fn file_read_grant_scope_hash(scope: &FileReadGrantScope<'_>) -> String {
    hash_text(&format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        scope.subject_actor_id,
        scope.workspace_id,
        scope.audience,
        scope.operation,
        scope.canonical_path.display(),
        scope.max_bytes,
        scope.declared_mime.unwrap_or(""),
        scope.resource.len,
        scope.resource.sha256
    ))
}

fn policy_decision_id(
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

fn hash_text(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

#[cfg(test)]
mod tests;
