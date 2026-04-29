use super::*;

#[test]
fn m1_session_search_indexes_redacted_manifest() {
    let mut index = Bm25CandidateIndex::new();
    let mut sessions = SessionSearchIndex::new();
    let scope = IndexScope::new("workspace-1", "account-1");

    // 1. 创建 RedactedSessionManifest
    let manifest = RedactedSessionManifest::new(
        "session-1",
        "user asked about rust ownership",
        scope.clone(),
        "ref://original-transcript-1",
    );

    // 2. 索引到 Bm25CandidateIndex
    sessions
        .index_redacted_session(&manifest, &mut index)
        .expect("index session");

    // 3. 搜索返回 candidate ids
    let results = sessions
        .search_sessions("rust", &scope, &index, 10)
        .expect("search succeeds");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, "session-1");

    // 4. 验证原始 transcript 不在索引中，只有 summary
    let sess_scope = session_scope(&scope);
    let doc = index
        .document(&sess_scope, &IndexCandidateId::new("session-1"))
        .expect("document exists");
    assert!(doc.body.contains("user asked about rust ownership"));
    assert!(!doc.body.contains("ref://original-transcript-1"));

    // 5. 验证 TTL 过期后先标记 expired，grace period 后物理删除
    let mut expired_manifest = RedactedSessionManifest::new(
        "session-2",
        "temporary session",
        scope.clone(),
        "ref://original-2",
    );
    expired_manifest.redacted_at = 0;
    expired_manifest.ttl_seconds = 10;
    sessions
        .index_redacted_session(&expired_manifest, &mut index)
        .expect("index expired session");

    // TTL 刚到期 -> 标记 expired
    let actions = sessions
        .cleanup_expired_sessions(15, &mut index)
        .expect("cleanup");
    assert_eq!(actions.len(), 1);
    assert!(
        matches!(&actions[0], SessionCleanupAction::MarkExpired { session_id } if session_id == "session-2")
    );

    // 索引中已不可搜索
    let results_after = sessions
        .search_sessions("temporary", &scope, &index, 10)
        .expect("search");
    assert!(results_after.is_empty());

    // grace period 后 -> 物理删除
    let actions = sessions
        .cleanup_expired_sessions(15 + 7 * 24 * 60 * 60 + 1, &mut index)
        .expect("cleanup");
    assert!(
        matches!(&actions[0], SessionCleanupAction::PhysicallyDelete { session_id, .. } if session_id == "session-2")
    );
    assert_eq!(sessions.entry_count(), 1); // session-1 仍在
}

// ---- M1 E2E: 低信任 Data Block 注入边界验证 ----
