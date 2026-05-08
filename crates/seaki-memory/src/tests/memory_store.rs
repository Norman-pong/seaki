use crate::memory_item::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, TrustLevel,
};
use crate::memory_store::{CapacityStatus, MemoryStore, MemoryStoreError};
use seaki_index::IndexScope;

fn scope() -> IndexScope {
    IndexScope::new("workspace-a", "account-a")
}

fn dummy_item(id: &str, proposed_at: u64) -> MemoryItem {
    MemoryItem {
        memory_id: id.to_string(),
        kind: MemoryKind::DerivedFact,
        scope: scope(),
        content: "content".to_string(),
        source_citation: None,
        proposed_at,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: None,
        status: MemoryStatus::Proposed,
        trust_level: TrustLevel::Unverified,
        confirmed_by: None,
        provenance: MemoryProvenance {
            origin: MemoryOrigin::SystemInferred,
            extraction_method: "test".to_string(),
            session_id: None,
            wiki_patch_hash: None,
        },
    }
}

#[test]
fn memory_store_propose_and_get() {
    let mut store = MemoryStore::new(10);
    let item = dummy_item("mem-1", 100);
    store.propose(item.clone()).unwrap();

    let got = store.get("mem-1").unwrap();
    assert_eq!(got.memory_id, "mem-1");
    assert_eq!(store.len(), 1);
}

#[test]
fn memory_store_capacity_limit_enforced() {
    let mut store = MemoryStore::new(2);
    store.propose(dummy_item("mem-1", 100)).unwrap();
    store.propose(dummy_item("mem-2", 200)).unwrap();

    // 第 3 个插入时触发淘汰：最旧的 mem-1（Proposed）被 Archive
    store.propose(dummy_item("mem-3", 300)).unwrap();

    assert_eq!(store.len(), 3);
    assert_eq!(store.get("mem-1").unwrap().status, MemoryStatus::Archived);
    // 3/2 = 150% >= 100%，容量状态为 Full（evict 只改状态不移除）
    assert_eq!(store.check_capacity(), CapacityStatus::Full);
}

#[test]
fn memory_store_transition_status() {
    let mut store = MemoryStore::new(10);
    let item = dummy_item("mem-1", 100);
    store.propose(item).unwrap();

    store
        .transition_status("mem-1", MemoryStatus::Scanning)
        .unwrap();
    store
        .transition_status("mem-1", MemoryStatus::SourceChecking)
        .unwrap();
    store
        .transition_status("mem-1", MemoryStatus::Approved)
        .unwrap();

    assert_eq!(store.get("mem-1").unwrap().status, MemoryStatus::Approved);
}

#[test]
fn memory_store_prune_expired() {
    let mut store = MemoryStore::new(10);
    let mut item = dummy_item("mem-1", 100);
    item.expires_at = Some(500);
    store.propose(item).unwrap();

    let expired = store.prune_expired(600);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].memory_id, "mem-1");
    assert_eq!(store.get("mem-1").unwrap().status, MemoryStatus::Expired);
}

#[test]
fn memory_store_evict_lru_when_full() {
    let mut store = MemoryStore::new(3);
    let mut active = dummy_item("mem-active", 50);
    active.status = MemoryStatus::Active;
    store.propose(active).unwrap();

    store.propose(dummy_item("mem-old", 100)).unwrap();
    store.propose(dummy_item("mem-new", 200)).unwrap();

    // 此时已满（3/3），再插入应淘汰最旧的非关键项 mem-old
    store.propose(dummy_item("mem-extra", 300)).unwrap();

    assert_eq!(store.get("mem-old").unwrap().status, MemoryStatus::Archived);
}

#[test]
fn memory_store_duplicate_id_rejected() {
    let mut store = MemoryStore::new(10);
    let item = dummy_item("mem-1", 100);
    store.propose(item.clone()).unwrap();

    let result = store.propose(item);
    assert!(matches!(
        result,
        Err(MemoryStoreError::DuplicateId(ref id)) if id == "mem-1"
    ));
}

#[test]
fn memory_store_invalid_transition_rejected() {
    let mut store = MemoryStore::new(10);
    let mut item = dummy_item("mem-1", 100);
    item.status = MemoryStatus::Active;
    store.propose(item).unwrap();

    let result = store.transition_status("mem-1", MemoryStatus::Proposed);
    assert!(matches!(
        result,
        Err(MemoryStoreError::InvalidTransition { from, to })
            if from == MemoryStatus::Active && to == MemoryStatus::Proposed
    ));
}

#[test]
fn memory_store_items_by_status_and_kind() {
    let mut store = MemoryStore::new(10);
    let mut a = dummy_item("mem-a", 100);
    a.kind = MemoryKind::UserPreference;
    a.status = MemoryStatus::Active;

    let mut b = dummy_item("mem-b", 200);
    b.kind = MemoryKind::ProjectConvention;
    b.status = MemoryStatus::Proposed;

    store.propose(a).unwrap();
    store.propose(b).unwrap();

    assert_eq!(store.items_by_status(MemoryStatus::Active).len(), 1);
    assert_eq!(store.items_by_kind(MemoryKind::UserPreference).len(), 1);
    assert_eq!(store.items_by_kind(MemoryKind::SafetyRule).len(), 0);
}

#[test]
fn memory_store_active_items_for_scope() {
    let mut store = MemoryStore::new(10);
    let mut a = dummy_item("mem-a", 100);
    a.status = MemoryStatus::Active;

    let mut b = dummy_item("mem-b", 200);
    b.status = MemoryStatus::Active;
    b.scope = IndexScope::new("other-ws", "other-ac");

    store.propose(a).unwrap();
    store.propose(b).unwrap();

    let active = store.active_items_for_scope(&scope());
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].memory_id, "mem-a");
}

#[test]
fn memory_store_capacity_status() {
    let mut store = MemoryStore::new(10);
    assert_eq!(store.check_capacity(), CapacityStatus::Healthy);

    for i in 0..8 {
        store
            .propose(dummy_item(&format!("mem-{i}"), i as u64))
            .unwrap();
    }
    // 8/10 = 80%，等于 80% 仍算 Healthy（>0.80 才 Warning）
    assert_eq!(store.check_capacity(), CapacityStatus::Healthy);

    store.propose(dummy_item("mem-8", 8)).unwrap();
    // 9/10 = 90% > 0.80，Warning
    assert_eq!(store.check_capacity(), CapacityStatus::Warning);

    store.propose(dummy_item("mem-9", 9)).unwrap();
    // 10/10 = 100%，Full
    assert_eq!(store.check_capacity(), CapacityStatus::Full);
}
