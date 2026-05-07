//! Approval gate for intercepting pipeline execution.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

use seaki_policy::ApprovalStatus;

/// Input for submitting an approval request.
pub struct ApprovalRequestInput {
    pub pipeline_id: String,
    pub step_id: String,
    pub actor_id: String,
    pub workspace_id: String,
    pub operation: String,
    pub reason: String,
}

/// Errors that can occur when interacting with an approval gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalGateError {
    RequestFailed(String),
    NotFound(String),
    Timeout {
        approval_id: String,
        timeout_ms: u64,
    },
}

impl std::fmt::Display for ApprovalGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestFailed(msg) => write!(f, "approval request failed: {msg}"),
            Self::NotFound(id) => write!(f, "approval request not found: {id}"),
            Self::Timeout {
                approval_id,
                timeout_ms,
            } => write!(
                f,
                "approval timeout: id={approval_id}, timeout={timeout_ms}ms"
            ),
        }
    }
}

impl std::error::Error for ApprovalGateError {}

impl From<ApprovalGateError> for crate::dry_run::PipelineError {
    fn from(_err: ApprovalGateError) -> Self {
        Self {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: crate::dry_run::ErrorKind::ComposeFailed,
        }
    }
}

/// Trait for approval gate implementations.
pub trait ApprovalGate: Send + Sync {
    /// Submit an approval request and return the approval ID.
    fn request_approval(&self, request: ApprovalRequestInput) -> Result<String, ApprovalGateError>;

    /// Poll the status of an approval request.
    fn poll_approval(&self, approval_id: &str) -> Result<ApprovalStatus, ApprovalGateError>;

    /// Block until the approval is resolved or timeout.
    fn wait_for_approval(
        &self,
        approval_id: &str,
        timeout_ms: u64,
    ) -> Result<ApprovalStatus, ApprovalGateError>;
}

/// In-memory approval gate for testing.
pub struct InMemoryApprovalGate {
    next_id: AtomicU64,
    requests: Mutex<HashMap<String, ApprovalStatus>>,
}

impl Default for InMemoryApprovalGate {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryApprovalGate {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            requests: Mutex::new(HashMap::new()),
        }
    }

    /// Manually approve a request (for testing).
    pub fn approve(&self, approval_id: &str) -> Result<(), ApprovalGateError> {
        let mut requests = self
            .requests
            .lock()
            .map_err(|e| ApprovalGateError::RequestFailed(e.to_string()))?;
        if requests.contains_key(approval_id) {
            requests.insert(approval_id.to_string(), ApprovalStatus::Approved);
            Ok(())
        } else {
            Err(ApprovalGateError::NotFound(approval_id.to_string()))
        }
    }

    /// Manually deny a request (for testing).
    pub fn deny(&self, approval_id: &str) -> Result<(), ApprovalGateError> {
        let mut requests = self
            .requests
            .lock()
            .map_err(|e| ApprovalGateError::RequestFailed(e.to_string()))?;
        if requests.contains_key(approval_id) {
            requests.insert(approval_id.to_string(), ApprovalStatus::Denied);
            Ok(())
        } else {
            Err(ApprovalGateError::NotFound(approval_id.to_string()))
        }
    }
}

impl ApprovalGate for InMemoryApprovalGate {
    fn request_approval(&self, request: ApprovalRequestInput) -> Result<String, ApprovalGateError> {
        let id = format!(
            "approval-{}-{}",
            request.pipeline_id,
            self.next_id.fetch_add(1, Ordering::SeqCst)
        );
        let mut requests = self
            .requests
            .lock()
            .map_err(|e| ApprovalGateError::RequestFailed(e.to_string()))?;
        requests.insert(id.clone(), ApprovalStatus::Pending);
        Ok(id)
    }

    fn poll_approval(&self, approval_id: &str) -> Result<ApprovalStatus, ApprovalGateError> {
        let requests = self
            .requests
            .lock()
            .map_err(|e| ApprovalGateError::RequestFailed(e.to_string()))?;
        requests
            .get(approval_id)
            .copied()
            .ok_or_else(|| ApprovalGateError::NotFound(approval_id.to_string()))
    }

    fn wait_for_approval(
        &self,
        approval_id: &str,
        timeout_ms: u64,
    ) -> Result<ApprovalStatus, ApprovalGateError> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        loop {
            match self.poll_approval(approval_id)? {
                ApprovalStatus::Approved => return Ok(ApprovalStatus::Approved),
                ApprovalStatus::Denied => return Ok(ApprovalStatus::Denied),
                ApprovalStatus::Pending => {
                    if start.elapsed() >= timeout {
                        return Err(ApprovalGateError::Timeout {
                            approval_id: approval_id.to_string(),
                            timeout_ms,
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    }
}
