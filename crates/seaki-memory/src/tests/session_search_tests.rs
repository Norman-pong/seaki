use crate::{
    redaction::RedactedSessionManifest, session_scope, SessionCleanupAction, SessionSearchIndex,
};
use seaki_index::{Bm25CandidateIndex, IndexCandidateId, IndexScope};

fn scope() -> IndexScope {
    IndexScope::new("workspace-a", "account-a")
}

fn manifest(id: &str, summary: &str) -> RedactedSessionManifest {
    RedactedSessionManifest::new(id, summary, scope(), format!("ref://{}", id))
}

#[test]
fn index_does_not_save_original_transcript() {
    let mut bm25 = Bm25CandidateIndex::new();
    let mut sessions = SessionSearchIndex::new();

    let m = manifest("s-1", "user asked about rust");
    sessions
        .index_redacted_session(m.clone(), &mut bm25)
        .unwrap();

    // 索引中只存在 summary，不存在 transcript ref
    let doc = bm25
        .get_document(&session_scope(&scope()), &IndexCandidateId::new("s-1"))
        .unwrap();
    assert!(doc.body.contains("user asked about rust"));
    assert!(!doc.body.contains("ref://"));
}

#[test]
fn search_returns_session_candidates() {
    let mut bm25 = Bm25CandidateIndex::new();
    let mut sessions = SessionSearchIndex::new();

    sessions
        .index_redacted_session(manifest("s-1", "rust ownership questions"), &mut bm25)
        .unwrap();
    sessions
        .index_redacted_session(manifest("s-2", "python async questions"), &mut bm25)
        .unwrap();

    let results = sessions
        .search_sessions("rust", &scope(), &bm25, 10)
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, "s-1");
    assert!(results[0].snippet.as_deref().unwrap().contains("rust"));
}

#[test]
fn ttl_expired_marked_then_deleted_after_grace_period() {
    let mut bm25 = Bm25CandidateIndex::new();
    let mut sessions = SessionSearchIndex::new();

    let mut m = manifest("s-1", "question");
    m.redacted_at = 0;
    m.ttl_seconds = 10; // 10 秒后过期
    sessions.index_redacted_session(m, &mut bm25).unwrap();

    // 第 1 阶段：TTL 刚到期 -> 标记 expired
    let actions = sessions.cleanup_expired_sessions(15, &mut bm25).unwrap();
    assert_eq!(actions.len(), 1);
    assert!(
        matches!(&actions[0], SessionCleanupAction::MarkExpired { session_id } if session_id == "s-1")
    );

    // 索引中应该已经不可搜索（Active 被重建）
    let results = sessions
        .search_sessions("question", &scope(), &bm25, 10)
        .unwrap();
    assert!(results.is_empty());

    // 第 2 阶段：7 天 grace period 后 -> 物理删除
    let actions = sessions
        .cleanup_expired_sessions(15 + 7 * 24 * 60 * 60 + 1, &mut bm25)
        .unwrap();
    assert!(
        matches!(&actions[0], SessionCleanupAction::PhysicallyDelete { session_id, .. } if session_id == "s-1")
    );
    assert_eq!(sessions.entry_count(), 0);
}

#[test]
fn session_scope_is_isolated_from_wiki_scope() {
    let base = IndexScope::new("ws", "ac");
    let sess = session_scope(&base);
    assert_eq!(sess.workspace_id, "ws");
    assert_eq!(sess.account_id, "ac:session");
}
