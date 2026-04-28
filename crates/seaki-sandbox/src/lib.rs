pub const PRIMARY_M0_BACKEND: &str = "macos-seatbelt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxProfile {
    ReadOnly,
    WorkspaceWrite,
    SourceIngest,
}

impl SandboxProfile {
    pub const fn allows_network(self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ingest_profile_is_networkless() {
        assert_eq!(PRIMARY_M0_BACKEND, "macos-seatbelt");
        assert!(!SandboxProfile::SourceIngest.allows_network());
    }
}
