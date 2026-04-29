pub mod note;
pub mod redaction;
pub mod session_search;

pub const SCHEMA_VERSION: u32 = 1;

pub use note::{
    memory_scope, NoteSearchResult, NoteStatus, NoteStore, NoteStoreError, ProjectNote,
};
pub use redaction::{redact_and_summarize, RedactedSessionManifest, RedactionStatus};
pub use session_search::{
    session_scope, SessionCleanupAction, SessionIndexStatus, SessionManifestEntry,
    SessionSearchCandidate, SessionSearchError, SessionSearchIndex,
};
