pub mod conflict_detector;
pub mod frozen_snapshot;
pub mod memory_collector;
pub mod memory_item;
pub mod memory_store;
pub mod note;
pub mod propose_pipeline;
pub mod redaction;
pub mod session_memory;
pub mod session_search;

pub const SCHEMA_VERSION: u32 = 1;

pub use conflict_detector::*;
pub use frozen_snapshot::*;
pub use memory_collector::*;
pub use memory_item::*;
pub use memory_store::*;
pub use note::{
    memory_scope, NoteSearchResult, NoteStatus, NoteStore, NoteStoreError, ProjectNote,
};
pub use propose_pipeline::*;
pub use redaction::{redact_and_summarize, RedactedSessionManifest, RedactionStatus};
pub use session_memory::*;
pub use session_search::{
    session_scope, SessionCleanupAction, SessionIndexStatus, SessionManifestEntry,
    SessionSearchCandidate, SessionSearchError, SessionSearchIndex,
};

#[cfg(test)]
mod tests;
