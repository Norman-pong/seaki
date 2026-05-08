//! MemoryItem: automatic memory model with full lifecycle status machine.

use seaki_index::IndexScope;

/// 自动收集的 memory 项，独立于手动 `ProjectNote`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryItem {
    pub memory_id: String,
    pub kind: MemoryKind,
    pub scope: IndexScope,
    pub content: String,
    pub source_citation: Option<String>,
    pub proposed_at: u64,
    pub confirmed_at: Option<u64>,
    pub last_verified_at: Option<u64>,
    pub expires_at: Option<u64>,
    pub status: MemoryStatus,
    pub trust_level: TrustLevel,
    pub confirmed_by: Option<String>,
    pub provenance: MemoryProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    UserPreference,
    ProjectConvention,
    WorkflowPattern,
    SafetyRule,
    DerivedFact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryStatus {
    Proposed,
    Scanning,
    SourceChecking,
    Approved,
    Rejected,
    Active,
    Stale,
    Conflict,
    Expired,
    Archived,
    Deleted,
}

impl MemoryStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryStatus::Proposed => "proposed",
            MemoryStatus::Scanning => "scanning",
            MemoryStatus::SourceChecking => "source_checking",
            MemoryStatus::Approved => "approved",
            MemoryStatus::Rejected => "rejected",
            MemoryStatus::Active => "active",
            MemoryStatus::Stale => "stale",
            MemoryStatus::Conflict => "conflict",
            MemoryStatus::Expired => "expired",
            MemoryStatus::Archived => "archived",
            MemoryStatus::Deleted => "deleted",
        }
    }

    #[must_use]
    pub fn can_transition_to(self, target: MemoryStatus) -> bool {
        matches!(
            (self, target),
            (MemoryStatus::Proposed, MemoryStatus::Scanning)
                | (MemoryStatus::Scanning, MemoryStatus::SourceChecking)
                | (
                    MemoryStatus::SourceChecking,
                    MemoryStatus::Approved | MemoryStatus::Rejected | MemoryStatus::Conflict
                )
                | (MemoryStatus::Approved, MemoryStatus::Active)
                | (
                    MemoryStatus::Active,
                    MemoryStatus::Stale | MemoryStatus::Conflict | MemoryStatus::Expired
                )
                | (
                    MemoryStatus::Stale,
                    MemoryStatus::Archived | MemoryStatus::Deleted | MemoryStatus::Active
                )
                | (
                    MemoryStatus::Conflict,
                    MemoryStatus::Stale | MemoryStatus::Archived | MemoryStatus::Deleted
                )
                | (
                    MemoryStatus::Expired,
                    MemoryStatus::Archived | MemoryStatus::Deleted
                )
                | (
                    MemoryStatus::Rejected,
                    MemoryStatus::Archived | MemoryStatus::Deleted
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    Unverified,
    Hint,
    Confirmed,
    Authority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProvenance {
    pub origin: MemoryOrigin,
    pub extraction_method: String,
    pub session_id: Option<String>,
    pub wiki_patch_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryOrigin {
    SessionHistory,
    WikiPatch,
    ApprovalDecision,
    UserExplicit,
    SystemInferred,
}
