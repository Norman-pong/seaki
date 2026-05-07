use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::audit::AuditRecord;
use crate::grant::CapabilityGrantRejection;

pub const CAPABILITY_GRANT_VISIBILITY: &str = "opaque-id-only";
pub const FILE_READ_CAPABILITY: &str = "file.read";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideEffectLevel {
    None,
    ProposalOnly,
    SideEffect,
}

impl SideEffectLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ProposalOnly => "proposal_only",
            Self::SideEffect => "side_effect",
        }
    }
}

impl std::fmt::Display for SideEffectLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SideEffectLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "proposal_only" => Ok(Self::ProposalOnly),
            "side_effect" => Ok(Self::SideEffect),
            other => Err(format!("unknown side_effect_level: {other}")),
        }
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
    pub(crate) fn allow(reason: PolicyReason, audit: AuditRecord) -> Self {
        Self {
            decision: PolicyDecision::Allow,
            reason,
            audit,
        }
    }

    pub(crate) fn deny(reason: PolicyReason, audit: AuditRecord) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
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
