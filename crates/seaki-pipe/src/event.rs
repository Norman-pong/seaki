//! Pipeline event stream: `PipelineEvent`, `EventSink`, `InMemoryEventSink`, `JsonlFileSink`.

use crate::ast::FrameType;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::Mutex;

/// Structured event emitted during real pipeline execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase")]
pub enum PipelineEvent {
    PipelineStarted {
        pipeline_id: String,
        input: serde_json::Value,
        timestamp_ms: u64,
    },
    StepStarted {
        step_id: String,
        command_id: String,
        timestamp_ms: u64,
    },
    Frame {
        step_id: String,
        seq: u64,
        frame_type: String,
        payload_summary: String,
    },
    CheckpointSaved {
        step_id: String,
        timestamp_ms: u64,
    },
    StepCompleted {
        step_id: String,
        output_frame_count: usize,
        timestamp_ms: u64,
    },
    StepFailed {
        step_id: String,
        error_kind: String,
        retryable: bool,
        timestamp_ms: u64,
    },
    ApprovalRequested {
        step_id: String,
        approval_id: String,
        reason: String,
        timestamp_ms: u64,
    },
    ApprovalDecided {
        step_id: String,
        approval_id: String,
        approved: bool,
        timestamp_ms: u64,
    },
    PipelineCompleted {
        pipeline_id: String,
        final_state: String,
        timestamp_ms: u64,
    },
}

impl PipelineEvent {
    /// Convenience constructor for a `Frame` event with truncated payload summary.
    #[must_use]
    pub fn frame(
        step_id: impl Into<String>,
        seq: u64,
        frame_type: &FrameType,
        payload: &serde_json::Value,
    ) -> Self {
        let payload_str = payload.to_string();
        let payload_summary = if payload_str.len() > 200 {
            format!("{}...", &payload_str[..200])
        } else {
            payload_str
        };
        Self::Frame {
            step_id: step_id.into(),
            seq,
            frame_type: frame_type.to_string(),
            payload_summary,
        }
    }
}

/// Errors that can occur when sinking events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventSinkError {
    IoError(String),
    SerializeError(String),
}

impl std::fmt::Display for EventSinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(msg) => write!(f, "event sink I/O error: {msg}"),
            Self::SerializeError(msg) => write!(f, "event sink serialization error: {msg}"),
        }
    }
}

impl std::error::Error for EventSinkError {}

/// Sink for collecting or persisting pipeline events.
pub trait EventSink: Send + Sync {
    /// Emit a single event into the sink.
    fn emit(&self, event: PipelineEvent);
    /// Ensure all buffered events are persisted.
    fn flush(&self) -> Result<(), EventSinkError>;
}

/// In-memory event sink for testing and frontend replay.
#[derive(Debug, Default)]
pub struct InMemoryEventSink {
    events: Mutex<Vec<PipelineEvent>>,
}

impl InMemoryEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a clone of all collected events.
    pub fn events(&self) -> Vec<PipelineEvent> {
        self.events
            .lock()
            .map_or_else(|_| Vec::new(), |guard| guard.clone())
    }

    /// Return events whose `step_id` matches the given id.
    pub fn events_for_step(&self, step_id: &str) -> Vec<PipelineEvent> {
        self.events()
            .into_iter()
            .filter(|e| e.step_id() == Some(step_id))
            .collect()
    }

    /// Serialize all events as a JSONL string (one JSON object per line, trailing newline).
    pub fn to_jsonl(&self) -> String {
        self.events()
            .into_iter()
            .filter_map(|e| match serde_json::to_string(&e) {
                Ok(line) => Some(line),
                Err(err) => {
                    eprintln!("Failed to serialize event: {err}");
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    /// Return the event sequence for replay.
    pub fn replay(&self) -> Vec<PipelineEvent> {
        self.events()
    }
}

impl EventSink for InMemoryEventSink {
    fn emit(&self, event: PipelineEvent) {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(event);
        }
    }

    fn flush(&self) -> Result<(), EventSinkError> {
        Ok(())
    }
}

/// File-backed JSONL event sink.
pub struct JsonlFileSink {
    #[allow(dead_code)]
    path: std::path::PathBuf,
    file: Mutex<std::fs::File>,
}

impl JsonlFileSink {
    /// Open (or create) a JSONL file for append.
    ///
    /// # Errors
    /// Returns `EventSinkError::IoError` if the file cannot be opened or created.
    pub fn open(path: impl Into<std::path::PathBuf>) -> Result<Self, EventSinkError> {
        let path = path.into();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| EventSinkError::IoError(e.to_string()))?;
        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }
}

impl EventSink for JsonlFileSink {
    fn emit(&self, event: PipelineEvent) {
        let json = match serde_json::to_string(&event) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("JsonlFileSink: serialize error: {e}");
                return;
            }
        };
        if let Ok(mut guard) = self.file.lock() {
            if let Err(e) = writeln!(guard, "{json}") {
                eprintln!("JsonlFileSink: write error: {e}");
            }
        }
    }

    fn flush(&self) -> Result<(), EventSinkError> {
        let guard = self
            .file
            .lock()
            .map_err(|e| EventSinkError::IoError(e.to_string()))?;
        guard
            .sync_all()
            .map_err(|e| EventSinkError::IoError(e.to_string()))?;
        Ok(())
    }
}

/// Helper to extract `step_id` from any event variant.
impl PipelineEvent {
    fn step_id(&self) -> Option<&str> {
        match self {
            Self::StepStarted { step_id, .. }
            | Self::Frame { step_id, .. }
            | Self::CheckpointSaved { step_id, .. }
            | Self::StepCompleted { step_id, .. }
            | Self::StepFailed { step_id, .. }
            | Self::ApprovalRequested { step_id, .. }
            | Self::ApprovalDecided { step_id, .. } => Some(step_id.as_str()),
            Self::PipelineStarted { .. } | Self::PipelineCompleted { .. } => None,
        }
    }
}
