use crate::memory_item::MemoryStatus;

#[test]
fn memory_status_transitions_valid() {
    assert!(MemoryStatus::Proposed.can_transition_to(MemoryStatus::Scanning));
    assert!(MemoryStatus::Scanning.can_transition_to(MemoryStatus::SourceChecking));
    assert!(MemoryStatus::SourceChecking.can_transition_to(MemoryStatus::Approved));
    assert!(MemoryStatus::SourceChecking.can_transition_to(MemoryStatus::Rejected));
    assert!(MemoryStatus::SourceChecking.can_transition_to(MemoryStatus::Conflict));
    assert!(MemoryStatus::Approved.can_transition_to(MemoryStatus::Active));
    assert!(MemoryStatus::Active.can_transition_to(MemoryStatus::Stale));
    assert!(MemoryStatus::Active.can_transition_to(MemoryStatus::Conflict));
    assert!(MemoryStatus::Active.can_transition_to(MemoryStatus::Expired));
    assert!(MemoryStatus::Stale.can_transition_to(MemoryStatus::Archived));
    assert!(MemoryStatus::Stale.can_transition_to(MemoryStatus::Deleted));
    assert!(MemoryStatus::Stale.can_transition_to(MemoryStatus::Active));
    assert!(MemoryStatus::Conflict.can_transition_to(MemoryStatus::Stale));
    assert!(MemoryStatus::Conflict.can_transition_to(MemoryStatus::Archived));
    assert!(MemoryStatus::Conflict.can_transition_to(MemoryStatus::Deleted));
    assert!(MemoryStatus::Expired.can_transition_to(MemoryStatus::Archived));
    assert!(MemoryStatus::Expired.can_transition_to(MemoryStatus::Deleted));
    assert!(MemoryStatus::Rejected.can_transition_to(MemoryStatus::Archived));
    assert!(MemoryStatus::Rejected.can_transition_to(MemoryStatus::Deleted));
}

#[test]
fn memory_status_transitions_invalid() {
    // Proposed 不能直接到 Approved
    assert!(!MemoryStatus::Proposed.can_transition_to(MemoryStatus::Approved));
    // Active 不能直接到 Approved
    assert!(!MemoryStatus::Active.can_transition_to(MemoryStatus::Approved));
    // Deleted 是终态，不能转移到任何状态
    assert!(!MemoryStatus::Deleted.can_transition_to(MemoryStatus::Archived));
    assert!(!MemoryStatus::Deleted.can_transition_to(MemoryStatus::Active));
    // Rejected 不能直接到 Active
    assert!(!MemoryStatus::Rejected.can_transition_to(MemoryStatus::Active));
    // Expired 不能直接到 Active
    assert!(!MemoryStatus::Expired.can_transition_to(MemoryStatus::Active));
    // Archived 是终态
    assert!(!MemoryStatus::Archived.can_transition_to(MemoryStatus::Deleted));
}
