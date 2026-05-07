use crate::{
    CompactionError, MessageRole, Session, SessionClaim, SessionCompactor, SessionMessage,
};

fn make_session(message_count: usize) -> Session {
    let messages: Vec<SessionMessage> = (0..message_count)
        .map(|i| SessionMessage {
            seq: i as u64,
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content: format!("message {i}"),
            timestamp_ms: i as u64 * 1000,
            metadata: serde_json::Value::Null,
        })
        .collect();

    Session {
        session_id: "test-session".to_string(),
        workspace_id: "test-workspace".to_string(),
        actor_id: "test-actor".to_string(),
        messages,
        claims: vec![],
        created_at_ms: 0,
        updated_at_ms: 0,
    }
}

#[test]
fn compaction_nothing_to_compact() {
    let mut session = make_session(10);
    let compactor = SessionCompactor::new();
    assert!(matches!(
        compactor.compact(&mut session).unwrap_err(),
        CompactionError::NothingToCompact
    ));
}

#[test]
fn compaction_reduces_message_count() {
    let mut session = make_session(30);
    let compactor = SessionCompactor::new();
    let summary = compactor.compact(&mut session).unwrap();
    assert!(session.messages.len() < 30);
    assert_eq!(summary.original_message_count, 30);
    assert!(summary.removed_message_count > 0);
}

#[test]
fn compaction_retains_recent_messages() {
    let mut session = make_session(30);
    let compactor = SessionCompactor::new();
    let original = session.messages.clone();
    compactor.compact(&mut session).unwrap();

    let recent_count = compactor.max_messages_before_compact / 2;
    let recent_start = original.len() - recent_count;
    for (i, msg) in original.iter().enumerate().skip(recent_start) {
        assert!(
            session.messages.iter().any(|m| m.content == msg.content),
            "recent message {i} should be retained"
        );
    }
}

#[test]
fn compaction_retains_decision_messages() {
    let mut session = make_session(30);
    // Insert a decision keyword into an older message.
    session.messages[5].content = "this requires approval from user".to_string();

    let compactor = SessionCompactor::new();
    compactor.compact(&mut session).unwrap();

    let found = session
        .messages
        .iter()
        .any(|m| m.content.contains("approval"));
    assert!(found, "decision message should be retained");
}

#[test]
fn compaction_retains_claims() {
    let mut session = make_session(30);
    session.claims.push(SessionClaim {
        claim_id: "c1".to_string(),
        text: "claim text".to_string(),
        source_seq: 1,
        confidence: 0.9,
    });

    let compactor = SessionCompactor::new();
    let summary = compactor.compact(&mut session).unwrap();
    assert_eq!(session.claims.len(), 1);
    assert_eq!(summary.retained_claim_count, 1);
}

#[test]
fn compaction_summary_inserted() {
    let mut session = make_session(30);
    let compactor = SessionCompactor::new();
    compactor.compact(&mut session).unwrap();

    assert_eq!(session.messages[0].role, MessageRole::System);
    assert!(session.messages[0].content.starts_with("[Session summary:"));
}
