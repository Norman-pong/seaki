//! `ChannelResourceGrant` and `ChannelActionGrant` models.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelAttachmentRef {
    pub attachment_id: String,
    pub file_key: String,
    pub version: String,
    pub original_name: String,
    pub claimed_mime: String,
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
}

/// Simulated file broker that "downloads" attachments into a quarantine path.
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
            self.quarantine_root, attachment.file_key, attachment.version
        );
        QuarantinedDownload {
            file_key: attachment.file_key.clone(),
            version: attachment.version.clone(),
            quarantine_path,
            observed_mime: attachment.claimed_mime.clone(),
            content_hash: format!("sha256:mock-{}-{}", attachment.file_key, attachment.version),
            malware_scan_status: MalwareScanStatus::Clean,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelResourceGrant {
    pub grant_id: String,
    pub scope: String,
    pub file_key: String,
    pub version: String,
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
        }
    }
}

impl std::error::Error for GrantError {}

/// In-memory store for `ChannelResourceGrant`.
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

    /// Consume one use of a grant after validating scope, `file_key` and version.
    pub fn consume(
        &self,
        grant_id: &str,
        scope: &str,
        file_key: &str,
        version: &str,
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

        grant.uses_remaining -= 1;
        Ok(())
    }

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
