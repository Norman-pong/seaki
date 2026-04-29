use crate::{memory_scope, NoteStatus, NoteStore, NoteStoreError, ProjectNote};
use seaki_index::{Bm25CandidateIndex, CandidateKind, IndexCandidateId, IndexScope};

fn scope() -> IndexScope {
    IndexScope::new("workspace-a", "account-a")
}

fn store_with_note(title: &str, content: &str) -> (NoteStore, ProjectNote) {
    let mut store = NoteStore::new();
    let note = store.create_note(title.to_string(), content, &scope());
    (store, note)
}

#[test]
fn note_lifecycle_from_proposed_to_active() {
    let (mut store, note) = store_with_note("title", "content");
    assert_eq!(note.status, NoteStatus::Proposed);

    store
        .transition_status(&note.note_id, NoteStatus::Scanning)
        .unwrap();
    store
        .transition_status(&note.note_id, NoteStatus::SourceChecking)
        .unwrap();
    store
        .transition_status(&note.note_id, NoteStatus::Approved)
        .unwrap();
    store
        .transition_status(&note.note_id, NoteStatus::Active)
        .unwrap();

    let n = store.get_note(&note.note_id).unwrap();
    assert_eq!(n.status, NoteStatus::Active);
}

#[test]
fn invalid_status_transition_is_rejected() {
    let (mut store, note) = store_with_note("title", "content");
    assert!(matches!(
        store.transition_status(&note.note_id, NoteStatus::Active),
        Err(NoteStoreError::InvalidStatusTransition { .. })
    ));
}

#[test]
fn note_is_searchable_via_bm25() {
    let mut store = NoteStore::new();
    let note = store.create_note(
        "rust ownership".to_string(),
        "ownership and borrowing in rust",
        &scope(),
    );

    let mut bm25 = Bm25CandidateIndex::new();
    store.rebuild_index(&mut bm25, &scope()).unwrap();

    let results = store.search_notes("ownership", &scope(), &bm25, 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].note_id, note.note_id);
    assert_eq!(results[0].status, NoteStatus::Proposed);
}

#[test]
fn source_conflict_downgrades_note_to_conflict() {
    let (mut store, note) = store_with_note("claim note", "this content overlaps with claim");
    store
        .transition_status(&note.note_id, NoteStatus::Scanning)
        .unwrap();
    store
        .transition_status(&note.note_id, NoteStatus::SourceChecking)
        .unwrap();

    let conflict = store
        .check_source_conflicts(
            &note.note_id,
            &["overlaps".to_string(), "claim".to_string()],
        )
        .unwrap();
    assert!(conflict);

    let n = store.get_note(&note.note_id).unwrap();
    assert_eq!(n.status, NoteStatus::Conflict);
}

#[test]
fn no_conflict_allows_approval() {
    let (mut store, note) = store_with_note("safe note", "completely unrelated content");
    store
        .transition_status(&note.note_id, NoteStatus::Scanning)
        .unwrap();
    store
        .transition_status(&note.note_id, NoteStatus::SourceChecking)
        .unwrap();

    let conflict = store
        .check_source_conflicts(&note.note_id, &["bitcoin".to_string()])
        .unwrap();
    assert!(!conflict);

    // 仍然可以进入 Approved
    store
        .transition_status(&note.note_id, NoteStatus::Approved)
        .unwrap();
}

#[test]
fn note_has_no_citation_ref() {
    let mut store = NoteStore::new();
    let note = store.create_note("t".to_string(), "b", &scope());
    let mut bm25 = Bm25CandidateIndex::new();
    store.rebuild_index(&mut bm25, &scope()).unwrap();

    let doc = bm25
        .get_document(
            &memory_scope(&scope()),
            &IndexCandidateId::new(&note.note_id),
        )
        .unwrap();
    assert!(doc.citation_ref.is_none());
    assert_eq!(doc.kind, CandidateKind::MemoryNote);
}

#[test]
fn memory_scope_is_isolated() {
    let base = IndexScope::new("ws", "ac");
    let mem = memory_scope(&base);
    assert_eq!(mem.account_id, "ac:memory");
}
