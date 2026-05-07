use crate::{SessionState, SessionStateMachine};

#[test]
fn session_idle_to_planning() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    assert_eq!(sm.state, SessionState::Idle);
    assert!(sm
        .transition(SessionState::Planning, "user submitted intent")
        .is_ok());
    assert_eq!(sm.state, SessionState::Planning);
}

#[test]
fn session_planning_to_executing() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    sm.transition(SessionState::Planning, "user submitted intent")
        .unwrap();
    assert!(sm
        .transition(SessionState::Executing, "pipeline proposal ready")
        .is_ok());
    assert_eq!(sm.state, SessionState::Executing);
}

#[test]
fn session_executing_to_awaiting_user() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    sm.transition(SessionState::Planning, "").unwrap();
    sm.transition(SessionState::Executing, "").unwrap();
    assert!(sm
        .transition(SessionState::AwaitingUser, "needs approval")
        .is_ok());
    assert_eq!(sm.state, SessionState::AwaitingUser);
}

#[test]
fn session_awaiting_user_to_executing() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    sm.transition(SessionState::Planning, "").unwrap();
    sm.transition(SessionState::Executing, "").unwrap();
    sm.transition(SessionState::AwaitingUser, "").unwrap();
    assert!(sm
        .transition(SessionState::Executing, "user approved")
        .is_ok());
    assert_eq!(sm.state, SessionState::Executing);
}

#[test]
fn session_executing_to_compacting() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    sm.transition(SessionState::Planning, "").unwrap();
    sm.transition(SessionState::Executing, "").unwrap();
    assert!(sm
        .transition(SessionState::Compacting, "execution done, compacting")
        .is_ok());
    assert_eq!(sm.state, SessionState::Compacting);
}

#[test]
fn session_compacting_to_idle() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    sm.transition(SessionState::Planning, "").unwrap();
    sm.transition(SessionState::Executing, "").unwrap();
    sm.transition(SessionState::Compacting, "").unwrap();
    assert!(sm.transition(SessionState::Idle, "compaction done").is_ok());
    assert_eq!(sm.state, SessionState::Idle);
}

#[test]
fn session_invalid_transition_rejected() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    assert!(sm.transition(SessionState::Compacting, "invalid").is_err());
    assert_eq!(sm.state, SessionState::Idle);
}

#[test]
fn session_transition_history_recorded() {
    let mut sm = SessionStateMachine::new("s1".to_string());
    sm.transition(SessionState::Planning, "p").unwrap();
    sm.transition(SessionState::Executing, "e").unwrap();
    sm.transition(SessionState::Idle, "reset").unwrap();
    assert_eq!(sm.history.len(), 3);
    assert_eq!(sm.history[0].from, SessionState::Idle);
    assert_eq!(sm.history[0].to, SessionState::Planning);
    assert_eq!(sm.history[0].reason, "p");
    assert_eq!(sm.history[1].from, SessionState::Planning);
    assert_eq!(sm.history[1].to, SessionState::Executing);
    assert_eq!(sm.history[2].from, SessionState::Executing);
    assert_eq!(sm.history[2].to, SessionState::Idle);
}
