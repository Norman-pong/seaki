use serde::{Deserialize, Serialize};

/// A single turn in the agent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMessage {
    pub seq: u64,
    pub role: crate::llm::MessageRole,
    pub content: String,
    pub timestamp_ms: u64,
    pub metadata: serde_json::Value,
}

/// A claim extracted from session history (for compaction retention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionClaim {
    pub claim_id: String,
    pub text: String,
    pub source_seq: u64, // which message this claim came from
    pub confidence: f32,
}

/// Agent session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub workspace_id: String,
    pub actor_id: String,
    pub messages: Vec<SessionMessage>,
    pub claims: Vec<SessionClaim>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    /// Timeout in milliseconds for approval wait. Default: 30000.
    pub approval_timeout_ms: u64,
}

// ---------------------------------------------------------------------------
// Session State Machine
// ---------------------------------------------------------------------------

/// Lifecycle states of an agent session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    Planning,
    Executing,
    AwaitingUser,
    Compacting,
}

/// A recorded state transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransition {
    pub from: SessionState,
    pub to: SessionState,
    pub reason: String,
    pub timestamp_ms: u64,
}

/// Error returned when an illegal state transition is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransitionError {
    pub from: SessionState,
    pub to: SessionState,
    pub reason: String,
}

impl std::fmt::Display for StateTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid state transition from {:?} to {:?}: {}",
            self.from, self.to, self.reason
        )
    }
}

impl std::error::Error for StateTransitionError {}

/// Manages valid state transitions for a session.
pub struct SessionStateMachine {
    pub session_id: String,
    pub state: SessionState,
    pub history: Vec<StateTransition>,
}

impl SessionStateMachine {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            state: SessionState::Idle,
            history: Vec::new(),
        }
    }

    pub fn is_in_state(&self, state: SessionState) -> bool {
        self.state == state
    }

    /// Attempt to transition to `to` with the given `reason`.
    pub fn transition(
        &mut self,
        to: SessionState,
        reason: &str,
    ) -> Result<(), StateTransitionError> {
        let from = self.state;

        if !Self::is_valid_transition(from, to) {
            return Err(StateTransitionError {
                from,
                to,
                reason: format!("transition from {:?} to {:?} is not allowed", from, to),
            });
        }

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.state = to;
        self.history.push(StateTransition {
            from,
            to,
            reason: reason.to_string(),
            timestamp_ms,
        });

        Ok(())
    }

    fn is_valid_transition(from: SessionState, to: SessionState) -> bool {
        match (from, to) {
            (SessionState::Idle, SessionState::Planning) => true,
            (SessionState::Idle, SessionState::Executing) => true,
            (SessionState::Planning, SessionState::Executing) => true,
            (SessionState::Planning, SessionState::AwaitingUser) => true,
            (SessionState::Executing, SessionState::AwaitingUser) => true,
            (SessionState::Executing, SessionState::Idle) => true,
            (SessionState::Executing, SessionState::Compacting) => true,
            (SessionState::AwaitingUser, SessionState::Planning) => true,
            (SessionState::AwaitingUser, SessionState::Executing) => true,
            (SessionState::Compacting, SessionState::Idle) => true,
            // Any non-Idle state -> Idle for reset / error recovery.
            (from, SessionState::Idle) if from != SessionState::Idle => true,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Session Compaction
// ---------------------------------------------------------------------------

/// Summary produced by a compaction run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionSummary {
    pub original_message_count: usize,
    pub removed_message_count: usize,
    pub retained_claim_count: usize,
    pub summary_text: String,
    pub compacted_at_ms: u64,
}

/// Errors that can occur during compaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionError {
    NothingToCompact,
    SummaryTooLong { max_len: usize, actual_len: usize },
}

impl std::fmt::Display for CompactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompactionError::NothingToCompact => write!(f, "nothing to compact"),
            CompactionError::SummaryTooLong {
                max_len,
                actual_len,
            } => {
                write!(f, "summary too long: max {max_len}, actual {actual_len}")
            }
        }
    }
}

impl std::error::Error for CompactionError {}

/// Compacts a session by summarizing old messages and retaining key claims.
pub struct SessionCompactor {
    pub max_messages_before_compact: usize,
    pub max_claims_to_retain: usize,
}

impl Default for SessionCompactor {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionCompactor {
    pub fn new() -> Self {
        Self {
            max_messages_before_compact: 20,
            max_claims_to_retain: 50,
        }
    }

    /// Compact the session. Returns the compaction summary.
    ///
    /// Logic:
    /// 1. If messages.len() <= max_messages_before_compact, return NothingToCompact.
    /// 2. Mark messages containing decision keywords ("approval", "decision", "proposal", "reject")
    ///    as must_retain.
    /// 3. Mark the most recent `max_messages_before_compact / 2` messages as must_retain.
    /// 4. All must_retain messages are kept in original order; the rest are summarized.
    /// 5. A System message with the summary is inserted at the front of messages.
    pub fn compact(&self, session: &mut Session) -> Result<CompactionSummary, CompactionError> {
        if session.messages.len() <= self.max_messages_before_compact {
            return Err(CompactionError::NothingToCompact);
        }

        let original_message_count = session.messages.len();
        let retained_claim_count = session.claims.len();

        let decision_keywords = ["approval", "decision", "proposal", "reject"];

        // Step 2: scan for decision messages.
        let mut must_retain = vec![false; session.messages.len()];
        for (i, msg) in session.messages.iter().enumerate() {
            let content_lower = msg.content.to_lowercase();
            if decision_keywords
                .iter()
                .any(|kw| content_lower.contains(kw))
            {
                must_retain[i] = true;
            }
        }

        // Step 3: retain recent messages.
        let recent_retain_count = self.max_messages_before_compact / 2;
        let recent_start = session.messages.len().saturating_sub(recent_retain_count);
        for flag in must_retain.iter_mut().skip(recent_start) {
            *flag = true;
        }

        // Step 4: split messages into retained / removed.
        let mut retained_messages = Vec::new();
        let mut removed_messages = Vec::new();
        for (i, msg) in session.messages.drain(..).enumerate() {
            if must_retain[i] {
                retained_messages.push(msg);
            } else {
                removed_messages.push(msg);
            }
        }

        // Build summary text from removed messages.
        let mut summary_parts = Vec::new();
        for msg in &removed_messages {
            summary_parts.push(format!("{:?}:{}", msg.role, msg.content));
        }
        let full_summary = summary_parts.join("\n");
        let summary_text = if full_summary.chars().count() > 500 {
            crate::safe_truncate(&full_summary, 500)
        } else {
            full_summary
        };

        let compacted_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Step 5: prepend System summary message with unique seq.
        let max_seq = session.messages.iter().map(|m| m.seq).max().unwrap_or(0);
        let summary_msg = SessionMessage {
            seq: max_seq + 1,
            role: crate::llm::MessageRole::System,
            content: format!("[Session summary: {summary_text}]"),
            timestamp_ms: compacted_at_ms,
            metadata: serde_json::Value::Null,
        };
        retained_messages.insert(0, summary_msg);
        session.messages = retained_messages;

        let removed_message_count = removed_messages.len();

        Ok(CompactionSummary {
            original_message_count,
            removed_message_count,
            retained_claim_count,
            summary_text,
            compacted_at_ms,
        })
    }
}
