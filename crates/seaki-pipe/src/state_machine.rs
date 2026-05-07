//! Pipeline state machine for lifecycle management.

/// Pipeline lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    Pending,
    Running,
    AwaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

/// Events that drive state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateEvent {
    Start,
    StepStarted { step_id: String },
    ApprovalRequested { approval_id: String },
    ApprovalGranted,
    ApprovalDenied,
    ApprovalTimeout,
    StepCompleted { step_id: String },
    StepFailed { step_id: String, retryable: bool },
    CompensateCompleted,
    Cancel,
    Complete,
}

/// Error returned when an invalid state transition is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransitionError {
    pub from: PipelineState,
    pub event: StateEvent,
    pub reason: String,
}

impl std::fmt::Display for StateTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid transition from {:?} on event {:?}: {}",
            self.from, self.event, self.reason
        )
    }
}

impl std::error::Error for StateTransitionError {}

impl From<StateTransitionError> for crate::dry_run::PipelineError {
    fn from(_err: StateTransitionError) -> Self {
        Self {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: crate::dry_run::ErrorKind::ComposeFailed,
        }
    }
}

/// Drives a pipeline through its lifecycle states.
pub struct PipelineStateMachine {
    pub pipeline_id: String,
    pub state: PipelineState,
    pub current_step_id: Option<String>,
    pub approval_request_id: Option<String>,
}

impl PipelineStateMachine {
    pub fn new(pipeline_id: String) -> Self {
        Self {
            pipeline_id,
            state: PipelineState::Pending,
            current_step_id: None,
            approval_request_id: None,
        }
    }

    pub fn transition(&mut self, event: StateEvent) -> Result<(), StateTransitionError> {
        match (self.state, &event) {
            (PipelineState::Pending, StateEvent::Start) => {
                self.state = PipelineState::Running;
                Ok(())
            }
            (PipelineState::Running, StateEvent::StepStarted { step_id }) => {
                self.current_step_id = Some(step_id.clone());
                Ok(())
            }
            (PipelineState::Running, StateEvent::ApprovalRequested { approval_id }) => {
                self.state = PipelineState::AwaitingApproval;
                self.approval_request_id = Some(approval_id.clone());
                Ok(())
            }
            (PipelineState::Running, StateEvent::StepCompleted { .. }) => Ok(()),
            (
                PipelineState::Running,
                StateEvent::StepFailed {
                    retryable: false, ..
                },
            ) => {
                self.state = PipelineState::Failed;
                Ok(())
            }
            (
                PipelineState::Running,
                StateEvent::StepFailed {
                    retryable: true, ..
                },
            ) => Ok(()),
            (PipelineState::Running, StateEvent::Cancel) => {
                self.state = PipelineState::Cancelled;
                Ok(())
            }
            (PipelineState::Running, StateEvent::Complete) => {
                self.state = PipelineState::Completed;
                Ok(())
            }
            (PipelineState::AwaitingApproval, StateEvent::ApprovalGranted) => {
                self.state = PipelineState::Running;
                self.approval_request_id = None;
                Ok(())
            }
            (PipelineState::AwaitingApproval, StateEvent::ApprovalDenied) => {
                self.state = PipelineState::Failed;
                self.approval_request_id = None;
                Ok(())
            }
            (PipelineState::AwaitingApproval, StateEvent::ApprovalTimeout) => {
                self.state = PipelineState::Failed;
                self.approval_request_id = None;
                Ok(())
            }
            (PipelineState::Failed, StateEvent::CompensateCompleted) => Ok(()),
            (PipelineState::Cancelled, StateEvent::CompensateCompleted) => Ok(()),
            _ => Err(StateTransitionError {
                from: self.state,
                event: event.clone(),
                reason: format!("no transition defined for {:?} + {:?}", self.state, event),
            }),
        }
    }
}
