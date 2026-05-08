use crate::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, MemoryStore,
    SessionMemoryManager, TrustLevel,
};
use seaki_index::IndexScope;

fn make_item(
    memory_id: &str,
    kind: MemoryKind,
    scope: &IndexScope,
    content: &str,
    trust_level: TrustLevel,
    status: MemoryStatus,
) -> MemoryItem {
    MemoryItem {
        memory_id: memory_id.to_string(),
        kind,
        scope: scope.clone(),
        content: content.to_string(),
        source_citation: Some("test-source".to_string()),
        proposed_at: 1000,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: None,
        status,
        trust_level,
        confirmed_by: None,
        provenance: MemoryProvenance {
            origin: MemoryOrigin::UserExplicit,
            extraction_method: "test".to_string(),
            session_id: None,
            wiki_patch_hash: None,
        },
    }
}

#[test]
fn session_manager_starts_with_snapshot() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::ProjectConvention,
        &scope,
        "content1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
    );
    store.propose(item).unwrap();

    let mut manager = SessionMemoryManager::new();
    let snapshot = manager.start_session("session-1", &scope, &store, TrustLevel::Hint, 2000);

    assert_eq!(snapshot.session_id, "session-1");
    assert_eq!(snapshot.total_items, 1);
    assert!(manager.snapshot("session-1").is_some());
}

#[test]
fn session_manager_mid_session_write_not_in_snapshot() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::ProjectConvention,
        &scope,
        "content1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
    );
    store.propose(item).unwrap();

    let mut manager = SessionMemoryManager::new();
    let snapshot = manager.start_session("session-1", &scope, &store, TrustLevel::Hint, 2000);
    let initial_items = snapshot.total_items;

    let new_item = make_item(
        "m2",
        MemoryKind::UserPreference,
        &scope,
        "new pref",
        TrustLevel::Confirmed,
        MemoryStatus::Proposed,
    );
    manager.write_during_session("session-1", new_item).unwrap();

    let snapshot_after = manager.snapshot("session-1").unwrap();
    assert_eq!(snapshot_after.total_items, initial_items);
    assert_eq!(manager.mid_session_writes("session-1").unwrap().len(), 1);
}

#[test]
fn session_manager_end_session_persists_writes() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::ProjectConvention,
        &scope,
        "content1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
    );
    store.propose(item).unwrap();

    let mut manager = SessionMemoryManager::new();
    manager.start_session("session-1", &scope, &store, TrustLevel::Hint, 2000);

    let new_item = make_item(
        "m2",
        MemoryKind::UserPreference,
        &scope,
        "new pref",
        TrustLevel::Confirmed,
        MemoryStatus::Proposed,
    );
    manager.write_during_session("session-1", new_item).unwrap();

    let persisted = manager.end_session("session-1", &mut store).unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(store.len(), 2);
    assert!(manager.snapshot("session-1").is_none());
}

#[test]
fn session_manager_cleanup_removes_records() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::ProjectConvention,
        &scope,
        "content1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
    );
    store.propose(item).unwrap();

    let mut manager = SessionMemoryManager::new();
    manager.start_session("session-1", &scope, &store, TrustLevel::Hint, 2000);

    let new_item = make_item(
        "m2",
        MemoryKind::UserPreference,
        &scope,
        "new pref",
        TrustLevel::Confirmed,
        MemoryStatus::Proposed,
    );
    manager.write_during_session("session-1", new_item).unwrap();

    manager.cleanup_session("session-1");
    assert!(manager.snapshot("session-1").is_none());
    assert!(manager.mid_session_writes("session-1").is_none());
}

#[test]
fn session_manager_snapshot_not_found() {
    let manager = SessionMemoryManager::new();
    assert!(manager.snapshot("nonexistent").is_none());
}
