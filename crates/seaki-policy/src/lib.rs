pub mod audit;
pub mod engine;
pub mod grant;
pub mod path;
pub mod types;

// Re-exports for backward-compatible public API
pub use audit::AuditRecord;
pub use engine::{
    ApprovalDecision, ApprovalRequest, CapabilityPolicyRequest, FileReadPolicyRequest, PolicyEngine,
};
pub use grant::{
    CapabilityConsumption, CapabilityGrant, CapabilityGrantHandle, CapabilityGrantRejection,
    CapabilityStore, CapabilityUseFailure, ChannelActionGrant, ChannelActionGrantConsumption,
    FileReadGrantInput, FileResourceSnapshot, GenericCapabilityGrant, GenericUseCapabilityRequest,
    IssueChannelActionGrantInput, Provenance, UseCapabilityRequest,
};
pub use path::{WorkspacePathDecision, WorkspacePathPolicy};
pub use types::{
    ApprovalStatus, GrantError, PolicyDecision, PolicyError, PolicyEvaluation, PolicyReason,
    PolicyResult, SideEffectLevel, CAPABILITY_GRANT_VISIBILITY, FILE_READ_CAPABILITY,
};

// Crate-level shared utilities
pub(crate) fn hash_text(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}

#[cfg(test)]
mod tests;
