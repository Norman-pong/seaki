//! Checkpoint storage and retry logic.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::ast::{DagPipeline, DagStep};
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::registry::CommandRegistry;
use crate::run::{
    execute_step, now_ms, CommandExecutor, ExecutionContext, ResourceUsage, StepPolicy, StepState,
};
use crate::ErrorKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    StorageError(String),
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StorageError(msg) => write!(f, "checkpoint storage error: {msg}"),
        }
    }
}

impl std::error::Error for CheckpointError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub pipeline_id: String,
    pub step_id: String,
    pub output_frames: Vec<FrameEnvelope>,
    pub step_state: StepState,
    pub resource_used: ResourceUsage,
    pub timestamp_ms: u64,
}

pub trait CheckpointStore: Send + Sync {
    fn save(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError>;
    fn load_latest(&self, pipeline_id: &str) -> Result<Option<Checkpoint>, CheckpointError>;
    fn load_all(&self, pipeline_id: &str) -> Result<Vec<Checkpoint>, CheckpointError>;
    fn clear(&self, pipeline_id: &str) -> Result<(), CheckpointError>;
}

#[derive(Debug, Default)]
pub struct InMemoryCheckpointStore {
    data: Mutex<HashMap<String, Vec<Checkpoint>>>,
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn save(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        let mut data = self
            .data
            .lock()
            .map_err(|e| CheckpointError::StorageError(e.to_string()))?;
        data.entry(checkpoint.pipeline_id.clone())
            .or_default()
            .push(checkpoint.clone());
        Ok(())
    }

    fn load_latest(&self, pipeline_id: &str) -> Result<Option<Checkpoint>, CheckpointError> {
        let data = self
            .data
            .lock()
            .map_err(|e| CheckpointError::StorageError(e.to_string()))?;
        Ok(data.get(pipeline_id).and_then(|v| v.last().cloned()))
    }

    fn load_all(&self, pipeline_id: &str) -> Result<Vec<Checkpoint>, CheckpointError> {
        let data = self
            .data
            .lock()
            .map_err(|e| CheckpointError::StorageError(e.to_string()))?;
        Ok(data.get(pipeline_id).cloned().unwrap_or_default())
    }

    fn clear(&self, pipeline_id: &str) -> Result<(), CheckpointError> {
        let mut data = self
            .data
            .lock()
            .map_err(|e| CheckpointError::StorageError(e.to_string()))?;
        data.remove(pipeline_id);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_ms: u64,
}

pub(crate) fn execute_step_with_retry(
    step: &crate::ast::ComposedStep,
    input_frames: Vec<FrameEnvelope>,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
    retry_policy: &RetryPolicy,
) -> Result<Vec<FrameEnvelope>, PipelineError> {
    let mut last_error = None;
    for attempt in 1..=retry_policy.max_attempts {
        match execute_step(step, input_frames.clone(), registry, executors, policy, ctx) {
            Ok(frames) => return Ok(frames),
            Err(err) => {
                if !err.retryable {
                    return Err(err);
                }
                if attempt < retry_policy.max_attempts {
                    std::thread::sleep(std::time::Duration::from_millis(retry_policy.backoff_ms));
                }
                last_error = Some(err);
            }
        }
    }
    Err(last_error.expect("last_error set when retry exhausted"))
}

pub(crate) fn save_checkpoint(
    store: &dyn CheckpointStore,
    dag: &DagPipeline,
    step: &DagStep,
    output_frames: &[FrameEnvelope],
    step_state: StepState,
    ctx: &ExecutionContext,
) -> Result<(), PipelineError> {
    store
        .save(&Checkpoint {
            pipeline_id: dag.pipeline_id.clone(),
            step_id: step.composed.step_id.clone(),
            output_frames: output_frames.to_vec(),
            step_state,
            resource_used: ctx.resource_used.clone(),
            timestamp_ms: now_ms(),
        })
        .map_err(|_| PipelineError {
            retryable: false,
            failed_step_id: step.composed.step_id.clone(),
            error_kind: ErrorKind::ComposeFailed,
        })
}
