pub const RAW_SOURCE_STORAGE: &str = "append-only-content-addressed";

pub use seaki_dto::ImportStage as SourceIngestState;

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
