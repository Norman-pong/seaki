use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

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
    pub fn new(workspace_root: impl AsRef<Path>) -> PolicyResult<Self> {
        let workspace_root = canonicalize_existing(workspace_root.as_ref())?;
        Ok(Self {
            allow_roots: vec![workspace_root.clone()],
            deny_roots: Vec::new(),
            workspace_root,
        })
    }

    pub fn with_allow_roots(
        mut self,
        allow_roots: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> PolicyResult<Self> {
        self.allow_roots = canonicalize_roots(allow_roots)?;
        Ok(self)
    }

    pub fn with_deny_roots(
        mut self,
        deny_roots: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> PolicyResult<Self> {
        self.deny_roots = canonicalize_roots(deny_roots)?;
        Ok(self)
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn canonicalize_path(&self, path: impl AsRef<Path>) -> PolicyResult<PathBuf> {
        canonicalize_existing(path.as_ref())
    }

    pub fn is_workspace_read_allowed(&self, canonical_path: &Path) -> bool {
        self.is_allowlisted(canonical_path) && !self.is_denied(canonical_path)
    }

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

#[derive(Debug, Default)]
pub struct CapabilityStore {
    grants: Mutex<HashMap<String, CapabilityGrant>>,
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
    pub fn new() -> Self {
        Self::default()
    }

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
        let expected_scope_hash = file_read_grant_scope_hash(FileReadGrantScope {
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

    pub fn uses_remaining(&self, capability_id: &str) -> PolicyResult<Option<u32>> {
        let grants = self
            .grants
            .lock()
            .map_err(|_| PolicyError::CapabilityStorePoisoned)?;
        Ok(grants.get(capability_id).map(|grant| grant.uses_remaining))
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
    pub fn new(workspace_policy: WorkspacePathPolicy) -> Self {
        Self {
            workspace_policy,
            capability_store: CapabilityStore::new(),
            fixed_now: None,
        }
    }

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

    pub fn authorize_file_read(
        &self,
        request: FileReadPolicyRequest,
    ) -> PolicyResult<PolicyEvaluation> {
        let now = self.now();
        let workspace_decision = self
            .workspace_policy
            .classify_workspace_read(&request.path)?;
        if workspace_decision.decision.permits_side_effect() {
            return Ok(PolicyEvaluation::allow(
                PolicyReason::WorkspaceAllowlist,
                AuditRecord::policy_decision(
                    &request,
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
                    &request,
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
                    &request,
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
                    &request,
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
                        &request,
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

fn file_read_grant_scope_hash(scope: FileReadGrantScope<'_>) -> String {
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
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn policy_default_shape_keeps_grants_opaque() {
        assert_eq!(CAPABILITY_GRANT_VISIBILITY, "opaque-id-only");
        assert!(PolicyDecision::Allow.permits_side_effect());
        assert!(!PolicyDecision::Deny.permits_side_effect());
        assert!(!PolicyDecision::RequireApproval.permits_side_effect());
    }

    #[test]
    fn workspace_external_path_is_denied_by_default() {
        let fixture = Fixture::new();
        let external_file = fixture.write_external_file("outside.txt", "secret");
        let engine = fixture.engine();

        let evaluation = engine
            .authorize_file_read(fixture.request(external_file, None))
            .expect("policy evaluation");

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert_eq!(evaluation.reason, PolicyReason::PathOutsideWorkspace);
    }

    #[test]
    fn symlink_escape_is_denied() {
        let fixture = Fixture::new();
        let external_file = fixture.write_external_file("outside.txt", "secret");
        let symlink_path = fixture.workspace.path().join("link-outside.txt");
        create_symlink(&external_file, &symlink_path);
        let engine = fixture.engine();

        let evaluation = engine
            .authorize_file_read(fixture.request(symlink_path, None))
            .expect("policy evaluation");

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert_eq!(evaluation.reason, PolicyReason::PathOutsideWorkspace);
    }

    #[test]
    fn workspace_denylist_overrides_allowlist() {
        let fixture = Fixture::new();
        let denied_dir = fixture.workspace.path().join("private");
        fs::create_dir(&denied_dir).expect("create denied dir");
        let denied_file = denied_dir.join("note.md");
        fs::write(&denied_file, "secret").expect("write denied file");
        let policy = WorkspacePathPolicy::new(fixture.workspace.path())
            .expect("workspace policy")
            .with_deny_roots([denied_dir])
            .expect("deny roots");
        let engine = PolicyEngine::new(policy);

        let evaluation = engine
            .authorize_file_read(fixture.request(denied_file, None))
            .expect("policy evaluation");

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert_eq!(evaluation.reason, PolicyReason::PathDenied);
    }

    #[test]
    fn workspace_denylist_cannot_be_bypassed_by_grant() {
        let fixture = Fixture::new();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let denied_dir = fixture.workspace.path().join("private");
        fs::create_dir(&denied_dir).expect("create denied dir");
        let denied_file = denied_dir.join("note.md");
        fs::write(&denied_file, "secret").expect("write denied file");
        let policy = WorkspacePathPolicy::new(fixture.workspace.path())
            .expect("workspace policy")
            .with_deny_roots([denied_dir])
            .expect("deny roots");
        let engine = PolicyEngine::with_fixed_now(policy, now);
        engine
            .capability_store()
            .issue_file_read_grant(fixture.grant_input(&denied_file, now))
            .expect("issue grant")
            .expect("approved grant");

        let evaluation = engine
            .authorize_file_read(fixture.request_at(&denied_file, Some("cap-source"), now))
            .expect("policy evaluation");

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert_eq!(evaluation.reason, PolicyReason::PathDenied);
        assert_eq!(
            engine
                .capability_store()
                .uses_remaining("cap-source")
                .expect("uses remaining"),
            Some(1)
        );
    }

    #[test]
    fn default_deny_roots_apply_to_directories_created_after_policy_init() {
        let fixture = Fixture::new();
        let engine = fixture.engine();
        let denied_dir = fixture.workspace.path().join(".seaki");
        fs::create_dir(&denied_dir).expect("create denied dir after policy init");
        let denied_file = denied_dir.join("secret");
        fs::write(&denied_file, "secret").expect("write denied file");

        let evaluation = engine
            .authorize_file_read(fixture.request(denied_file, None))
            .expect("policy evaluation");

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert_eq!(evaluation.reason, PolicyReason::PathDenied);
    }

    #[test]
    fn grant_is_single_use_and_bound_to_audience() {
        let fixture = Fixture::new();
        let external_file = fixture.write_external_file("source.md", "# source");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let engine = fixture.engine_at(now);
        engine
            .capability_store()
            .issue_file_read_grant(fixture.grant_input(&external_file, now))
            .expect("issue grant")
            .expect("approved grant");

        let wrong_audience = engine
            .authorize_file_read(FileReadPolicyRequest {
                audience: "seaki-other".to_string(),
                capability_id: Some("cap-source".to_string()),
                ..fixture.request_at(&external_file, None, now)
            })
            .expect("wrong audience evaluation");
        assert_eq!(wrong_audience.decision, PolicyDecision::Deny);
        assert_eq!(
            wrong_audience.reason,
            PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::WrongAudience)
        );
        assert_eq!(
            engine
                .capability_store()
                .uses_remaining("cap-source")
                .expect("uses remaining"),
            Some(1)
        );

        let allowed = engine
            .authorize_file_read(fixture.request_at(&external_file, Some("cap-source"), now))
            .expect("allowed evaluation");
        assert_eq!(allowed.decision, PolicyDecision::Allow);
        assert_eq!(allowed.reason, PolicyReason::CapabilityGrant);
        assert_eq!(
            engine
                .capability_store()
                .uses_remaining("cap-source")
                .expect("uses remaining"),
            Some(0)
        );

        let reused = engine
            .authorize_file_read(fixture.request_at(&external_file, Some("cap-source"), now))
            .expect("reuse evaluation");
        assert_eq!(reused.decision, PolicyDecision::Deny);
        assert_eq!(
            reused.reason,
            PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::AlreadyUsed)
        );
    }

    #[test]
    fn expired_grant_is_rejected_without_consuming_use() {
        let fixture = Fixture::new();
        let external_file = fixture.write_external_file("source.md", "# source");
        let issued_at = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let engine = fixture.engine_at(issued_at + Duration::from_secs(61));
        engine
            .capability_store()
            .issue_file_read_grant(fixture.grant_input(&external_file, issued_at))
            .expect("issue grant")
            .expect("approved grant");

        let evaluation = engine
            .authorize_file_read(fixture.request_at(&external_file, Some("cap-source"), issued_at))
            .expect("expired evaluation");

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert_eq!(
            evaluation.reason,
            PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::Expired)
        );
        assert_eq!(
            engine
                .capability_store()
                .uses_remaining("cap-source")
                .expect("uses remaining"),
            Some(1)
        );
    }

    #[test]
    fn denied_approval_cannot_issue_file_read_grant() {
        let fixture = Fixture::new();
        let external_file = fixture.write_external_file("source.md", "# source");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let store = CapabilityStore::new();

        let result = store
            .issue_file_read_grant(FileReadGrantInput {
                approval: fixture.approval(now, ApprovalStatus::Denied),
                ..fixture.grant_input(&external_file, now)
            })
            .expect("issue grant evaluation");

        assert_eq!(result, Err(CapabilityGrantRejection::ApprovalNotApproved));
        assert_eq!(
            store.uses_remaining("cap-source").expect("uses remaining"),
            None
        );
    }

    #[test]
    fn approval_scope_hash_must_match_grant_resource() {
        let fixture = Fixture::new();
        let approved_file = fixture.write_external_file("approved.md", "# approved");
        let other_file = fixture.write_external_file("other.md", "# other");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let store = CapabilityStore::new();

        let result = store
            .issue_file_read_grant(FileReadGrantInput {
                approval: fixture.approval_for(&approved_file, now, ApprovalStatus::Approved),
                ..fixture.grant_input(&other_file, now)
            })
            .expect("issue grant evaluation");

        assert_eq!(result, Err(CapabilityGrantRejection::ApprovalScopeMismatch));
        assert_eq!(
            store.uses_remaining("cap-source").expect("uses remaining"),
            None
        );
    }

    #[test]
    fn changed_resource_version_rejects_grant_use_without_consuming_it() {
        let fixture = Fixture::new();
        let external_file = fixture.write_external_file("source.md", "# source");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let engine = fixture.engine_at(now);
        engine
            .capability_store()
            .issue_file_read_grant(fixture.grant_input(&external_file, now))
            .expect("issue grant")
            .expect("approved grant");
        fs::write(&external_file, "# replaced source").expect("replace source");

        let evaluation = engine
            .authorize_file_read(fixture.request_at(&external_file, Some("cap-source"), now))
            .expect("policy evaluation");

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert_eq!(
            evaluation.reason,
            PolicyReason::CapabilityGrantRejected(CapabilityGrantRejection::ResourceChanged)
        );
        assert_eq!(
            engine
                .capability_store()
                .uses_remaining("cap-source")
                .expect("uses remaining"),
            Some(1)
        );
        assert!(evaluation.audit.grant_fingerprint.is_some());
    }

    #[test]
    fn concurrent_grant_reuse_allows_only_one_consumer() {
        let fixture = Fixture::new();
        let external_file = fixture.write_external_file("source.md", "# source");
        let policy = WorkspacePathPolicy::new(fixture.workspace.path()).expect("workspace policy");
        let store = CapabilityStore::new();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        store
            .issue_file_read_grant(fixture.grant_input(&external_file, now))
            .expect("issue grant")
            .expect("approved grant");
        let engine = Arc::new(PolicyEngine::with_capability_store_and_fixed_now(
            policy, store, now,
        ));
        let barrier = Arc::new(Barrier::new(2));

        let handles = (0..2)
            .map(|_| {
                let engine = Arc::clone(&engine);
                let barrier = Arc::clone(&barrier);
                let request = fixture.request_at(&external_file, Some("cap-source"), now);
                thread::spawn(move || {
                    barrier.wait();
                    engine
                        .authorize_file_read(request)
                        .expect("policy evaluation")
                        .decision
                })
            })
            .collect::<Vec<_>>();

        let decisions = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread joins"))
            .collect::<Vec<_>>();
        let allowed = decisions
            .iter()
            .filter(|decision| **decision == PolicyDecision::Allow)
            .count();
        let denied = decisions
            .iter()
            .filter(|decision| **decision == PolicyDecision::Deny)
            .count();

        assert_eq!(allowed, 1);
        assert_eq!(denied, 1);
    }

    struct Fixture {
        workspace: TempDir,
        external: TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                workspace: tempfile::tempdir().expect("workspace tempdir"),
                external: tempfile::tempdir().expect("external tempdir"),
            }
        }

        fn engine(&self) -> PolicyEngine {
            PolicyEngine::new(
                WorkspacePathPolicy::new(self.workspace.path()).expect("workspace policy"),
            )
        }

        fn engine_at(&self, now: SystemTime) -> PolicyEngine {
            PolicyEngine::with_fixed_now(
                WorkspacePathPolicy::new(self.workspace.path()).expect("workspace policy"),
                now,
            )
        }

        fn write_external_file(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.external.path().join(name);
            fs::write(&path, contents).expect("write external file");
            path
        }

        fn request(
            &self,
            path: impl Into<PathBuf>,
            capability_id: Option<&str>,
        ) -> FileReadPolicyRequest {
            self.request_at(
                path,
                capability_id,
                SystemTime::UNIX_EPOCH + Duration::from_secs(100),
            )
        }

        fn request_at(
            &self,
            path: impl Into<PathBuf>,
            capability_id: Option<&str>,
            _now: SystemTime,
        ) -> FileReadPolicyRequest {
            FileReadPolicyRequest {
                actor_id: "user-1".to_string(),
                workspace_id: "ws-1".to_string(),
                audience: "seaki-source-ingest".to_string(),
                operation: "source.ingest".to_string(),
                path: path.into(),
                capability_id: capability_id.map(str::to_string),
            }
        }

        fn grant_input(&self, path: impl AsRef<Path>, now: SystemTime) -> FileReadGrantInput {
            FileReadGrantInput {
                capability_id: "cap-source".to_string(),
                subject_actor_id: "user-1".to_string(),
                workspace_id: "ws-1".to_string(),
                audience: "seaki-source-ingest".to_string(),
                operation: "source.ingest".to_string(),
                path: path.as_ref().to_path_buf(),
                max_bytes: 1024,
                declared_mime: Some("text/markdown".to_string()),
                not_before: now - Duration::from_secs(1),
                expires_at: now + Duration::from_secs(60),
                granted_by: "local_user".to_string(),
                approval: self.approval_for(path.as_ref(), now, ApprovalStatus::Approved),
            }
        }

        fn approval(&self, now: SystemTime, status: ApprovalStatus) -> ApprovalDecision {
            let path = self.external.path().join("source.md");
            self.approval_for(&path, now, status)
        }

        fn approval_for(
            &self,
            path: impl AsRef<Path>,
            now: SystemTime,
            status: ApprovalStatus,
        ) -> ApprovalDecision {
            let canonical_path = path.as_ref().canonicalize().expect("canonical path");
            let resource = snapshot_file(&canonical_path, 1024)
                .expect("resource snapshot")
                .expect("resource within limit");
            ApprovalDecision {
                approval_id: "approval-source".to_string(),
                policy_decision_id: "policy-source".to_string(),
                scope_hash: file_read_grant_scope_hash(FileReadGrantScope {
                    subject_actor_id: "user-1",
                    workspace_id: "ws-1",
                    audience: "seaki-source-ingest",
                    operation: "source.ingest",
                    canonical_path: &canonical_path,
                    max_bytes: 1024,
                    declared_mime: Some("text/markdown"),
                    resource: &resource,
                }),
                decided_by: "local_user".to_string(),
                status,
                decided_at: now,
            }
        }
    }

    #[cfg(unix)]
    fn create_symlink(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create symlink");
    }

    #[cfg(windows)]
    fn create_symlink(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_file(target, link).expect("create symlink");
    }
}
