use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::audit::AuditRecord;
use crate::engine::ApprovalDecision;
use crate::path::canonicalize_existing;
use crate::types::FILE_READ_CAPABILITY;
use crate::types::{ApprovalStatus, GrantError, PolicyError, PolicyResult};
use crate::{hash_bytes, hash_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResourceSnapshot {
    len: u64,
    sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrant {
    pub(crate) capability_id: String,
    pub(crate) subject_actor_id: String,
    pub(crate) workspace_id: String,
    pub(crate) capability: String,
    pub(crate) audience: String,
    pub(crate) operation: String,
    pub(crate) canonical_path: PathBuf,
    pub(crate) resource: FileResourceSnapshot,
    pub(crate) max_bytes: u64,
    pub(crate) declared_mime: Option<String>,
    pub(crate) not_before: SystemTime,
    pub(crate) expires_at: SystemTime,
    pub(crate) uses_remaining: u32,
    pub(crate) granted_by: String,
    pub(crate) approval_id: String,
    pub(crate) policy_decision_id: String,
    pub(crate) revoked_at: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityGrantHandle {
    pub capability_id: String,
    pub audit: AuditRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericCapabilityGrant {
    pub(crate) capability_id: String,
    pub(crate) subject_actor_id: String,
    pub(crate) workspace_id: String,
    pub(crate) capability: String,
    pub(crate) audience: String,
    pub(crate) operation: String,
    pub(crate) not_before: SystemTime,
    pub(crate) expires_at: SystemTime,
    pub(crate) uses_remaining: u32,
    pub(crate) granted_by: String,
    pub(crate) policy_decision_id: String,
    pub(crate) revoked_at: Option<SystemTime>,
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
pub struct GenericUseCapabilityRequest {
    pub capability_id: String,
    pub subject_actor_id: String,
    pub audience: String,
    pub workspace_id: String,
    pub capability: String,
    pub operation: String,
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

#[derive(Debug, Default)]
pub struct CapabilityStore {
    grants: Mutex<HashMap<String, CapabilityGrant>>,
    generic_grants: Mutex<HashMap<String, GenericCapabilityGrant>>,
    channel_action_grants: Mutex<HashMap<String, ChannelActionGrant>>,
    actor_memory_scopes: Mutex<HashMap<(String, String), Vec<String>>>,
    actor_source_scopes: Mutex<HashMap<(String, String), Vec<String>>>,
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

    /// 签发通用能力授权。
    ///
    /// # Errors
    ///
    /// 当能力存储锁中毒或重复 ID 时返回错误。
    #[allow(clippy::too_many_arguments)]
    pub fn issue_capability_grant(
        &self,
        capability_id: String,
        subject_actor_id: String,
        workspace_id: String,
        capability: String,
        audience: String,
        operation: String,
        not_before: Option<SystemTime>,
        expires_at: Option<SystemTime>,
        uses_remaining: u32,
        granted_by: String,
    ) -> PolicyResult<Result<CapabilityGrantHandle, CapabilityGrantRejection>> {
        let now = SystemTime::now();
        let not_before = not_before.unwrap_or(now);
        let expires_at = expires_at.unwrap_or(now + Duration::from_secs(365 * 24 * 60 * 60));
        let policy_decision_id = hash_text(&format!(
            "generic\n{}\n{}\n{}\n{}\n{}\n{:?}",
            capability_id, subject_actor_id, workspace_id, capability, operation, now
        ));

        let grant = GenericCapabilityGrant {
            capability_id: capability_id.clone(),
            subject_actor_id,
            workspace_id,
            capability,
            audience,
            operation,
            not_before,
            expires_at,
            uses_remaining,
            granted_by,
            policy_decision_id,
            revoked_at: None,
        };
        let audit = AuditRecord::generic_grant_issued(&grant);

        let mut grants = self
            .generic_grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;

        if grants.contains_key(&grant.capability_id) {
            return Err(PolicyError::DuplicateCapabilityId(
                grant.capability_id.clone(),
            ));
        }

        grants.insert(grant.capability_id.clone(), grant);
        Ok(Ok(CapabilityGrantHandle {
            capability_id,
            audit,
        }))
    }

    /// 消耗通用能力授权。
    ///
    /// # Errors
    ///
    /// 当能力存储锁中毒时返回错误。
    pub fn consume_generic_grant(
        &self,
        request: &GenericUseCapabilityRequest,
    ) -> PolicyResult<Result<CapabilityConsumption, CapabilityUseFailure>> {
        let mut grants = self
            .generic_grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        let Some(grant) = grants.get_mut(&request.capability_id) else {
            return Ok(Err(CapabilityUseFailure {
                rejection: CapabilityGrantRejection::Unknown,
                grant_fingerprint: None,
            }));
        };
        let grant_fingerprint = generic_grant_fingerprint(grant);
        let reject = |rejection| {
            Ok(Err(CapabilityUseFailure {
                rejection,
                grant_fingerprint: Some(grant_fingerprint.clone()),
            }))
        };

        if grant.capability != request.capability {
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

        grant.uses_remaining -= 1;
        Ok(Ok(CapabilityConsumption { grant_fingerprint }))
    }

    /// 查询指定通用能力授权的剩余使用次数。
    ///
    /// # Errors
    ///
    /// 当能力存储锁中毒时返回错误。
    pub fn generic_uses_remaining(&self, capability_id: &str) -> PolicyResult<Option<u32>> {
        let grants = self
            .generic_grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        Ok(grants.get(capability_id).map(|grant| grant.uses_remaining))
    }

    /// 检查指定 actor 是否拥有任何有效的通用能力授权。
    ///
    /// # Errors
    ///
    /// 当能力存储锁中毒时返回错误。
    pub fn has_valid_generic_grant(
        &self,
        subject_actor_id: &str,
        workspace_id: &str,
        capability: &str,
        operation: &str,
        audience: &str,
        now: SystemTime,
    ) -> PolicyResult<bool> {
        let grants = self
            .generic_grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        Ok(grants.values().any(|grant| {
            grant.subject_actor_id == subject_actor_id
                && grant.workspace_id == workspace_id
                && grant.capability == capability
                && grant.operation == operation
                && grant.audience == audience
                && grant.revoked_at.is_none()
                && grant.uses_remaining > 0
                && now >= grant.not_before
                && now < grant.expires_at
        }))
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

    /// 设置 actor 的 memory scopes。
    ///
    /// # Errors
    ///
    /// 当存储锁中毒时返回错误。
    pub fn set_memory_scopes(
        &self,
        actor_id: &str,
        workspace_id: &str,
        scopes: Vec<String>,
    ) -> PolicyResult<()> {
        let mut memory_scopes = self
            .actor_memory_scopes
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        memory_scopes.insert((actor_id.to_string(), workspace_id.to_string()), scopes);
        Ok(())
    }

    /// 设置 actor 的 source scopes。
    ///
    /// # Errors
    ///
    /// 当存储锁中毒时返回错误。
    pub fn set_source_scopes(
        &self,
        actor_id: &str,
        workspace_id: &str,
        scopes: Vec<String>,
    ) -> PolicyResult<()> {
        let mut source_scopes = self
            .actor_source_scopes
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        source_scopes.insert((actor_id.to_string(), workspace_id.to_string()), scopes);
        Ok(())
    }

    /// 检查 actor 是否拥有指定的 memory scope。
    ///
    /// # Errors
    ///
    /// 当存储锁中毒时返回错误。
    pub fn has_memory_scope(
        &self,
        actor_id: &str,
        workspace_id: &str,
        scope: &str,
    ) -> PolicyResult<bool> {
        let memory_scopes = self
            .actor_memory_scopes
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        Ok(memory_scopes
            .get(&(actor_id.to_string(), workspace_id.to_string()))
            .map(|scopes| scopes.contains(&scope.to_string()))
            .unwrap_or(false))
    }

    /// 检查 actor 是否拥有指定的 source scope。
    ///
    /// # Errors
    ///
    /// 当存储锁中毒时返回错误。
    pub fn has_source_scope(
        &self,
        actor_id: &str,
        workspace_id: &str,
        scope: &str,
    ) -> PolicyResult<bool> {
        let source_scopes = self
            .actor_source_scopes
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        Ok(source_scopes
            .get(&(actor_id.to_string(), workspace_id.to_string()))
            .map(|scopes| scopes.contains(&scope.to_string()))
            .unwrap_or(false))
    }
}

impl CapabilityGrant {
    pub(crate) fn fingerprint(&self) -> String {
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

pub(crate) fn snapshot_file(
    path: &Path,
    max_bytes: u64,
) -> PolicyResult<Option<FileResourceSnapshot>> {
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

pub(crate) struct FileReadGrantScope<'a> {
    pub(crate) subject_actor_id: &'a str,
    pub(crate) workspace_id: &'a str,
    pub(crate) audience: &'a str,
    pub(crate) operation: &'a str,
    pub(crate) canonical_path: &'a Path,
    pub(crate) max_bytes: u64,
    pub(crate) declared_mime: Option<&'a str>,
    pub(crate) resource: &'a FileResourceSnapshot,
}

pub(crate) fn file_read_grant_scope_hash(scope: &FileReadGrantScope<'_>) -> String {
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

pub(crate) fn generic_grant_fingerprint(grant: &GenericCapabilityGrant) -> String {
    hash_text(&format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
        grant.capability_id,
        grant.subject_actor_id,
        grant.workspace_id,
        grant.capability,
        grant.audience,
        grant.operation,
        grant
            .not_before
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        grant
            .expires_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        grant.granted_by,
    ))
}
