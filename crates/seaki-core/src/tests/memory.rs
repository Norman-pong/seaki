use super::*;

#[test]
fn memory_propose_creates_note_with_proposed_status() {
    let mut ledger = initialized_ledger();
    let envelope = ledger
        .append_memory_propose(memory_propose_request("event-2", "idem-2", "note-1"))
        .expect("memory propose succeeds");

    assert_eq!(envelope.event_type, MEMORY_PROPOSE_EVENT_TYPE);
    let note = ledger
        .memory_note("note-1")
        .expect("note loads")
        .expect("note exists");
    assert_eq!(note.status, "proposed");
    assert_eq!(note.title, "test-title");
}

#[test]
fn memory_propose_lifecycle_includes_source_checking() {
    let mut ledger = initialized_ledger();
    ledger
        .append_memory_propose(memory_propose_request("event-2", "idem-2", "note-1"))
        .expect("memory propose succeeds");

    // source_checking 无冲突 -> 状态变为 source_checking
    let note = ledger
        .memory_source_check("note-1", &["bitcoin".to_string()])
        .expect("source check succeeds");
    assert_eq!(note.status, "source_checking");
}

#[test]
fn memory_source_check_conflict_downgrades_note() {
    let mut ledger = initialized_ledger();
    ledger
        .append_memory_propose(memory_propose_request("event-2", "idem-2", "note-1"))
        .expect("memory propose succeeds");

    // source_checking 发现冲突 -> 降级为 conflict
    let note = ledger
        .memory_source_check("note-1", &["test content".to_string()])
        .expect("source check succeeds");
    assert_eq!(note.status, "conflict");
}

#[test]
fn memory_commit_requires_approved_decision() {
    let mut ledger = initialized_ledger();
    ledger
        .append_memory_propose(memory_propose_request("event-2", "idem-2", "note-1"))
        .expect("memory propose succeeds");

    let initial_events = ledger.event_count().expect("event count");
    assert!(matches!(
        ledger.append_memory_commit(memory_commit_request(
            "event-3",
            "idem-3",
            "note-1",
            "approval-1",
            3
        )),
        Err(CoreError::ApprovalDecisionRequired { .. })
    ));
    assert_eq!(ledger.event_count().expect("event count"), initial_events);
}

#[test]
fn memory_commit_activates_note_after_approval() {
    let mut ledger = initialized_ledger();
    ledger
        .append_memory_propose(memory_propose_request("event-2", "idem-2", "note-1"))
        .expect("memory propose succeeds");
    ledger
        .append_approval_decision(approval_decision_request(
            "event-3",
            "idem-3",
            "approval-1",
            "note-1",
            ApprovalDecisionStatus::Approved,
            None,
        ))
        .expect("approval decision appends");

    let envelope = ledger
        .append_memory_commit(memory_commit_request(
            "event-4",
            "idem-4",
            "note-1",
            "approval-1",
            4,
        ))
        .expect("memory commit succeeds");

    assert_eq!(envelope.event_type, MEMORY_COMMIT_EVENT_TYPE);
    let note = ledger
        .memory_note("note-1")
        .expect("note loads")
        .expect("note exists");
    assert_eq!(note.status, "active");
}

#[test]
fn m1_memory_note_lifecycle_with_source_checking() {
    let mut ledger = initialized_ledger();
    let mut index = Bm25CandidateIndex::new();
    let mut store = NoteStore::new();
    let scope = IndexScope::new("workspace-1", "account-1");

    // 1. 通过 CoreLedger 创建 project note（事件持久化）
    ledger
        .append_memory_propose(MemoryProposeRequest::new(
            "event-2",
            "actor-1",
            "workspace-1",
            "idem-2",
            "note-1",
            "rust ownership",
            "ownership and borrowing in rust",
        ))
        .expect("memory propose succeeds");

    let note = ledger
        .memory_note("note-1")
        .expect("note loads")
        .expect("note exists");
    assert_eq!(note.status, "proposed");
    assert_eq!(note.title, "rust ownership");

    // 2. 在 NoteStore 中创建对应 note 并重建 BM25 索引（memory scope 隔离）
    let store_note = store.create_note(
        "rust ownership".to_string(),
        "ownership and borrowing in rust",
        &scope,
    );
    store
        .rebuild_index(&mut index, &scope)
        .expect("rebuild index");

    // 3. 搜索 note，验证 BM25 返回结果
    let results = store.search_notes("ownership", &scope, &index, 10);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].note_id, store_note.note_id);
    assert_eq!(results[0].status, seaki_memory::note::NoteStatus::Proposed);

    // 4. 模拟 source_checking 冲突 -> 降级为 Conflict
    let conflict_note = ledger
        .memory_source_check("note-1", &["borrowing".to_string()])
        .expect("source check succeeds");
    assert_eq!(conflict_note.status, "conflict");

    // 5. 验证 note 不可被 citation 直接引用（citation_ref 为 null）
    let mem_scope = memory_scope(&scope);
    let doc = index
        .document(&mem_scope, &IndexCandidateId::new(&store_note.note_id))
        .expect("indexed document exists");
    assert!(doc.citation_ref.is_none());
    assert_eq!(doc.kind, CandidateKind::MemoryNote);
}

#[test]
fn m1_memory_propose_does_not_hot_replace_session_prompt() {
    let mut ledger = initialized_ledger();

    // 提交 memory.propose
    ledger
        .append_memory_propose(MemoryProposeRequest::new(
            "event-2",
            "actor-1",
            "workspace-1",
            "idem-2",
            "note-1",
            "tips",
            "use borrow checker",
        ))
        .expect("memory propose succeeds");

    // replay 所有事件
    let events = ledger.replay_events_after(0).expect("replay");

    // 验证只有 memory.proposed 事件，没有 prompt.replace 事件
    let memory_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == MEMORY_PROPOSE_EVENT_TYPE)
        .collect();
    assert_eq!(memory_events.len(), 1);

    let prompt_replace_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "prompt.replace")
        .collect();
    assert!(
        prompt_replace_events.is_empty(),
        "memory.propose must not emit prompt.replace events"
    );
}
