pub const RAW_SOURCE_STORAGE: &str = "append-only-content-addressed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceIngestState {
    Selected,
    GrantRequested,
    Granted,
    RawCommitted,
    ParseRunning,
    Parsed,
    Partial,
    Failed,
    PatchProposed,
    ApprovalPending,
    Committed,
    Denied,
    Indexed,
    IndexStale,
}

impl SourceIngestState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Parsed
                | Self::Partial
                | Self::Failed
                | Self::Committed
                | Self::Denied
                | Self::Indexed
                | Self::IndexStale
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiki_declares_append_only_source_layer() {
        assert_eq!(RAW_SOURCE_STORAGE, "append-only-content-addressed");
        assert!(SourceIngestState::Indexed.is_terminal());
        assert!(!SourceIngestState::ParseRunning.is_terminal());
    }
}
