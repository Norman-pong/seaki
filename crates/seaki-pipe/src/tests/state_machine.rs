use crate::state_machine::{PipelineState, PipelineStateMachine, StateEvent};

#[test]
fn state_machine_pending_to_running() {
    let mut sm = PipelineStateMachine::new("pipe-1".to_string());
    assert_eq!(sm.state, PipelineState::Pending);
    sm.transition(StateEvent::Start).unwrap();
    assert_eq!(sm.state, PipelineState::Running);
}

#[test]
fn state_machine_approval_granted_resumes() {
    let mut sm = PipelineStateMachine::new("pipe-1".to_string());
    sm.transition(StateEvent::Start).unwrap();
    sm.transition(StateEvent::ApprovalRequested {
        approval_id: "a1".to_string(),
    })
    .unwrap();
    assert_eq!(sm.state, PipelineState::AwaitingApproval);
    assert_eq!(sm.approval_request_id, Some("a1".to_string()));

    sm.transition(StateEvent::ApprovalGranted).unwrap();
    assert_eq!(sm.state, PipelineState::Running);
    assert_eq!(sm.approval_request_id, None);
}

#[test]
fn state_machine_approval_denied_fails() {
    let mut sm = PipelineStateMachine::new("pipe-1".to_string());
    sm.transition(StateEvent::Start).unwrap();
    sm.transition(StateEvent::ApprovalRequested {
        approval_id: "a1".to_string(),
    })
    .unwrap();
    sm.transition(StateEvent::ApprovalDenied).unwrap();
    assert_eq!(sm.state, PipelineState::Failed);
}

#[test]
fn state_machine_step_failure_non_retryable() {
    let mut sm = PipelineStateMachine::new("pipe-1".to_string());
    sm.transition(StateEvent::Start).unwrap();
    sm.transition(StateEvent::StepFailed {
        step_id: "s1".to_string(),
        retryable: false,
    })
    .unwrap();
    assert_eq!(sm.state, PipelineState::Failed);
}

#[test]
fn state_machine_cancel_while_running() {
    let mut sm = PipelineStateMachine::new("pipe-1".to_string());
    sm.transition(StateEvent::Start).unwrap();
    sm.transition(StateEvent::Cancel).unwrap();
    assert_eq!(sm.state, PipelineState::Cancelled);
}

#[test]
fn state_machine_invalid_transition_rejected() {
    let mut sm = PipelineStateMachine::new("pipe-1".to_string());
    sm.transition(StateEvent::Start).unwrap();
    sm.transition(StateEvent::Complete).unwrap();
    assert_eq!(sm.state, PipelineState::Completed);

    let result = sm.transition(StateEvent::Start);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.from, PipelineState::Completed);
    assert!(matches!(err.event, StateEvent::Start));
}
