use crate::{
    FrozenSnapshotBuilder, MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus,
    MemoryStore, SnapshotMemoryEntry, TrustLevel,
};
use seaki_index::IndexScope;

fn make_item(
    memory_id: &str,
    kind: MemoryKind,
    scope: &IndexScope,
    content: &str,
    trust_level: TrustLevel,
    status: MemoryStatus,
    expires_at: Option<u64>,
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
        expires_at,
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
fn snapshot_builds_from_active_items() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::ProjectConvention,
        &scope,
        "content1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        None,
    );
    store.propose(item).unwrap();

    let builder = FrozenSnapshotBuilder::new(&store);
    let snapshot = builder.build("session-1", &scope, TrustLevel::Hint, 2000);

    assert_eq!(snapshot.session_id, "session-1");
    assert_eq!(snapshot.total_items, 1);
    assert_eq!(snapshot.total_bytes, "content1".len());
    assert_eq!(snapshot.project_memories.len(), 1);
    assert!(snapshot.user_memories.is_empty());
}

#[test]
fn snapshot_excludes_expired_items() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::ProjectConvention,
        &scope,
        "content1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        Some(1500),
    );
    store.propose(item).unwrap();

    let builder = FrozenSnapshotBuilder::new(&store);
    let snapshot = builder.build("session-1", &scope, TrustLevel::Hint, 2000);

    assert_eq!(snapshot.total_items, 0);
    assert_eq!(snapshot.total_bytes, 0);
}

#[test]
fn snapshot_excludes_low_trust_items() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::ProjectConvention,
        &scope,
        "content1",
        TrustLevel::Unverified,
        MemoryStatus::Active,
        None,
    );
    store.propose(item).unwrap();

    let builder = FrozenSnapshotBuilder::new(&store);
    let snapshot = builder.build("session-1", &scope, TrustLevel::Hint, 2000);

    assert_eq!(snapshot.total_items, 0);
}

#[test]
fn snapshot_groups_by_kind() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let user_pref = make_item(
        "m1",
        MemoryKind::UserPreference,
        &scope,
        "pref1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        None,
    );
    let convention = make_item(
        "m2",
        MemoryKind::ProjectConvention,
        &scope,
        "conv1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        None,
    );
    let workflow = make_item(
        "m3",
        MemoryKind::WorkflowPattern,
        &scope,
        "wf1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        None,
    );
    let safety = make_item(
        "m4",
        MemoryKind::SafetyRule,
        &scope,
        "safe1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        None,
    );
    let derived = make_item(
        "m5",
        MemoryKind::DerivedFact,
        &scope,
        "fact1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        None,
    );

    store.propose(user_pref).unwrap();
    store.propose(convention).unwrap();
    store.propose(workflow).unwrap();
    store.propose(safety).unwrap();
    store.propose(derived).unwrap();

    let builder = FrozenSnapshotBuilder::new(&store);
    let snapshot = builder.build("session-1", &scope, TrustLevel::Hint, 2000);

    assert_eq!(snapshot.user_memories.len(), 1);
    assert_eq!(snapshot.project_memories.len(), 4);
    assert_eq!(snapshot.total_items, 5);
}

#[test]
fn snapshot_is_immutable_after_build() {
    let scope = IndexScope::new("ws1", "acc1");
    let mut store = MemoryStore::new(100);
    let item = make_item(
        "m1",
        MemoryKind::UserPreference,
        &scope,
        "pref1",
        TrustLevel::Confirmed,
        MemoryStatus::Active,
        None,
    );
    store.propose(item).unwrap();

    let builder = FrozenSnapshotBuilder::new(&store);
    let snapshot = builder.build("session-1", &scope, TrustLevel::Hint, 2000);
    let mut cloned = snapshot.clone();

    cloned.user_memories.push(SnapshotMemoryEntry {
        memory_id: "fake".to_string(),
        kind: MemoryKind::DerivedFact,
        content: "fake".to_string(),
        trust_level: TrustLevel::Unverified,
        source_citation: None,
    });

    assert_eq!(snapshot.user_memories.len(), 1);
    assert_eq!(cloned.user_memories.len(), 2);
}
