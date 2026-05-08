//! `ChannelResourceGrant` and `ChannelActionGrant` models.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelAttachmentRef {
    pub attachment_id: String,
    pub provider: String,
    pub provider_tenant_id: String,
    pub provider_chat_id: String,
    pub provider_message_id: String,
    pub provider_thread_id: String,
    pub provider_file_key: String,
    pub provider_file_version: String,
    pub original_name: String,
    pub declared_mime: String,
    pub declared_size: u64,
    pub content_hash: Option<String>,
    pub download_capability_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MalwareScanStatus {
    Clean,
    Suspicious,
    Infected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantinedDownload {
    pub file_key: String,
    pub version: String,
    pub quarantine_path: String,
    pub observed_mime: String,
    pub content_hash: String,
    pub malware_scan_status: MalwareScanStatus,
    pub observed_size: u64,
}

/// Simulated file broker that "downloads" attachments into a quarantine path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeBroker {
    quarantine_root: String,
}

impl FakeBroker {
    pub fn new(quarantine_root: impl Into<String>) -> Self {
        Self {
            quarantine_root: quarantine_root.into(),
        }
    }

    /// Mock download: returns quarantine metadata without real I/O.
    #[must_use]
    pub fn download(&self, attachment: &ChannelAttachmentRef) -> QuarantinedDownload {
        let quarantine_path = format!(
            "{}/{}_{}",
            self.quarantine_root, attachment.provider_file_key, attachment.provider_file_version
        );
        QuarantinedDownload {
            file_key: attachment.provider_file_key.clone(),
            version: attachment.provider_file_version.clone(),
            quarantine_path,
            observed_mime: attachment.declared_mime.clone(),
            content_hash: format!(
                "sha256:mock-{}-{}",
                attachment.provider_file_key, attachment.provider_file_version
            ),
            malware_scan_status: MalwareScanStatus::Clean,
            observed_size: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelResourceGrant {
    pub grant_id: String,
    pub scope: String,
    pub provider_tenant_id: String,
    pub provider_chat_id: String,
    pub provider_message_id: String,
    pub file_key: String,
    pub version: String,
    pub seaki_actor_id: String,
    pub operation: String,
    pub audience: String,
    pub idempotency_key: String,
    pub uses_remaining: u32,
    pub issued_at: SystemTime,
    pub expires_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantError {
    PolicyDeniedInsufficientRole,
    GrantNotFound,
    GrantExpired,
    UsesExhausted,
    ScopeMismatch,
    VersionMismatch,
    FileKeyMismatch,
    QuarantineFailed(String),
    MalwareDetected,
    MimeMismatch,
    HashMismatch,
    ActorMismatch,
    OperationMismatch,
    AudienceMismatch,
}

impl std::fmt::Display for GrantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PolicyDeniedInsufficientRole => write!(f, "POLICY_DENIED_INSUFFICIENT_ROLE"),
            Self::GrantNotFound => write!(f, "GRANT_NOT_FOUND"),
            Self::GrantExpired => write!(f, "GRANT_EXPIRED"),
            Self::UsesExhausted => write!(f, "USES_EXHAUSTED"),
            Self::ScopeMismatch => write!(f, "SCOPE_MISMATCH"),
            Self::VersionMismatch => write!(f, "VERSION_MISMATCH"),
            Self::FileKeyMismatch => write!(f, "FILE_KEY_MISMATCH"),
            Self::QuarantineFailed(msg) => write!(f, "QUARANTINE_FAILED: {msg}"),
            Self::MalwareDetected => write!(f, "MALWARE_DETECTED"),
            Self::MimeMismatch => write!(f, "MIME_MISMATCH"),
            Self::HashMismatch => write!(f, "HASH_MISMATCH"),
            Self::ActorMismatch => write!(f, "ACTOR_MISMATCH"),
            Self::OperationMismatch => write!(f, "OPERATION_MISMATCH"),
            Self::AudienceMismatch => write!(f, "AUDIENCE_MISMATCH"),
        }
    }
}

impl std::error::Error for GrantError {}

/// In-memory store for `ChannelResourceGrant`.
#[derive(Debug)]
pub struct ChannelResourceGrantStore {
    grants: Mutex<HashMap<String, ChannelResourceGrant>>,
}

impl ChannelResourceGrantStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
        }
    }

    /// Issue a grant.  Guests are rejected with `POLICY_DENIED_INSUFFICIENT_ROLE`.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns `GrantError::PolicyDeniedInsufficientRole` for guest roles.
    pub fn issue(
        &self,
        workspace_role: &str,
        grant: ChannelResourceGrant,
    ) -> Result<ChannelResourceGrant, GrantError> {
        if workspace_role == "guest" {
            return Err(GrantError::PolicyDeniedInsufficientRole);
        }
        let mut grants = self.grants.lock().unwrap();
        grants.insert(grant.grant_id.clone(), grant.clone());
        Ok(grant)
    }

    /// Consume one use of a grant after validating scope, `file_key`, version,
    /// `seaki_actor_id`, `operation` and `audience`.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns `GrantError` if the grant is missing, expired, exhausted, or mismatched.
    #[allow(clippy::too_many_arguments)]
    pub fn consume(
        &self,
        grant_id: &str,
        scope: &str,
        file_key: &str,
        version: &str,
        seaki_actor_id: &str,
        operation: &str,
        audience: &str,
        now: SystemTime,
    ) -> Result<(), GrantError> {
        let mut grants = self.grants.lock().unwrap();
        let grant = grants.get_mut(grant_id).ok_or(GrantError::GrantNotFound)?;

        if now >= grant.expires_at {
            return Err(GrantError::GrantExpired);
        }
        if grant.uses_remaining == 0 {
            return Err(GrantError::UsesExhausted);
        }
        if grant.scope != scope {
            return Err(GrantError::ScopeMismatch);
        }
        if grant.file_key != file_key {
            return Err(GrantError::FileKeyMismatch);
        }
        if grant.version != version {
            return Err(GrantError::VersionMismatch);
        }
        if grant.seaki_actor_id != seaki_actor_id {
            return Err(GrantError::ActorMismatch);
        }
        if grant.operation != operation {
            return Err(GrantError::OperationMismatch);
        }
        if grant.audience != audience {
            return Err(GrantError::AudienceMismatch);
        }

        grant.uses_remaining -= 1;
        Ok(())
    }

    /// Retrieve a grant by ID.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn get(&self, grant_id: &str) -> Option<ChannelResourceGrant> {
        let grants = self.grants.lock().unwrap();
        grants.get(grant_id).cloned()
    }
}

impl Default for ChannelResourceGrantStore {
    fn default() -> Self {
        Self::new()
    }
}
