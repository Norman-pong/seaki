use std::sync::Arc;
use std::time::Duration;

use seaki_policy::ApprovalStatus;

use crate::approval_gate::{
    ApprovalGate, ApprovalGateError, ApprovalRequestInput, InMemoryApprovalGate,
};

fn sample_request() -> ApprovalRequestInput {
    ApprovalRequestInput {
        pipeline_id: "pipe-1".to_string(),
        step_id: "s1".to_string(),
        actor_id: "actor-1".to_string(),
        workspace_id: "ws-1".to_string(),
        operation: "test.op".to_string(),
        reason: "test reason".to_string(),
    }
}

#[test]
fn approval_gate_request_returns_id() {
    let gate = InMemoryApprovalGate::new();
    let id = gate.request_approval(sample_request()).unwrap();
    assert!(!id.is_empty());
    assert!(id.starts_with("approval-pipe-1-"));
}

#[test]
fn approval_gate_poll_pending_then_approved() {
    let gate = InMemoryApprovalGate::new();
    let id = gate.request_approval(sample_request()).unwrap();

    assert_eq!(gate.poll_approval(&id).unwrap(), ApprovalStatus::Pending);

    gate.approve(&id).unwrap();
    assert_eq!(gate.poll_approval(&id).unwrap(), ApprovalStatus::Approved);
}

#[test]
fn approval_gate_wait_blocks_until_approved() {
    let gate = Arc::new(InMemoryApprovalGate::new());
    let id = gate.request_approval(sample_request()).unwrap();

    let gate2 = gate.clone();
    let approval_id = id.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        gate2.approve(&approval_id).unwrap();
    });

    let result = gate.wait_for_approval(&id, 5_000).unwrap();
    assert_eq!(result, ApprovalStatus::Approved);
}

#[test]
fn approval_gate_wait_timeout() {
    let gate = InMemoryApprovalGate::new();
    let id = gate.request_approval(sample_request()).unwrap();

    let result = gate.wait_for_approval(&id, 50);
    assert!(matches!(
        result,
        Err(ApprovalGateError::Timeout {
            approval_id,
            timeout_ms: 50,
        }) if approval_id == id
    ));
}

#[test]
fn approval_gate_denied_no_execute() {
    let gate = InMemoryApprovalGate::new();
    let id = gate.request_approval(sample_request()).unwrap();

    gate.deny(&id).unwrap();
    let status = gate.poll_approval(&id).unwrap();
    assert_eq!(status, ApprovalStatus::Denied);
}
