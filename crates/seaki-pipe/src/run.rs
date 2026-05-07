//! Real pipeline execution runtime.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use crate::ast::{
    Cardinality, ComposedPipeline, ComposedStep, DagMergeStrategy, DagNodeKind, DagPipeline,
    DagStep, FrameType, InputBinding,
};
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::registry::{CommandRegistry, ResourceQuota, SideEffectLevel};
use crate::ErrorKind;

/// Maximum number of frames allowed per step.
const MAX_FRAME_COUNT: u64 = 1_000;
/// Maximum frame payload size in bytes (1 MiB).
const MAX_FRAME_SIZE: u64 = 1_024 * 1_024;

pub struct ExecutionContext {
    pub workspace_id: String,
    pub actor_id: String,
    pub pipeline_id: String,
    pub audit: Vec<AuditRecord>,
    pub resource_used: ResourceUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub step_id: String,
    pub command_id: String,
    pub decision: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceUsage {
    pub cpu_ms: u64,
    pub memory_mb: u64,
    pub frame_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub output: Vec<FrameEnvelope>,
    pub audit: Vec<AuditRecord>,
}

pub use seaki_policy::PolicyDecision;

/// Runtime policy engine trait for per-step authorization checks.
pub trait StepPolicy: Send + Sync {
    /// Check whether a step is permitted to execute.
    fn check(&self, step: &ComposedStep, ctx: &ExecutionContext) -> PolicyDecision;
}

/// Placeholder policy implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimplePolicy;

impl StepPolicy for SimplePolicy {
    fn check(&self, step: &ComposedStep, _ctx: &ExecutionContext) -> PolicyDecision {
        match step.side_effect_level {
            SideEffectLevel::None => PolicyDecision::Allow,
            _ => PolicyDecision::RequireApproval,
        }
    }
}

pub trait CommandExecutor: Send + Sync {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError>;
}

/// Run a composed pipeline with real executors.
///
/// # Errors
/// Returns `PipelineError` if the pipeline is empty, a step fails and the
/// failure policy is `FailFast`, or a resource limit is exceeded.
pub fn run(
    pipeline: &ComposedPipeline,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
) -> Result<RunResult, PipelineError> {
    if pipeline.steps.is_empty() {
        return Err(PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        });
    }

    let mut step_outputs: HashMap<String, Vec<FrameEnvelope>> = HashMap::new();
    let mut previous_output: Vec<FrameEnvelope> = vec![FrameEnvelope {
        seq: 0,
        step_id: "input".to_string(),
        frame_type: pipeline.input_type.0,
        payload: initial_input,
    }];

    for step in &pipeline.steps {
        let input_frames = resolve_input(step, &previous_output, &step_outputs);
        let output_frames = execute_step(step, input_frames, registry, executors, policy, ctx)?;
        previous_output = output_frames.clone();
        step_outputs.insert(step.step_id.clone(), output_frames);
    }

    Ok(RunResult {
        output: previous_output,
        audit: ctx.audit.clone(),
    })
}

fn resolve_input(
    step: &ComposedStep,
    previous_output: &[FrameEnvelope],
    step_outputs: &HashMap<String, Vec<FrameEnvelope>>,
) -> Vec<FrameEnvelope> {
    match &step.input_binding {
        InputBinding::PreviousStep => previous_output.to_vec(),
        InputBinding::Constant(val) => {
            if step.input_type.1 == Cardinality::Many && val.is_array() {
                val.as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, v)| FrameEnvelope {
                        seq: i as u64,
                        step_id: step.step_id.clone(),
                        frame_type: step.input_type.0,
                        payload: v.clone(),
                    })
                    .collect()
            } else {
                vec![FrameEnvelope {
                    seq: 0,
                    step_id: step.step_id.clone(),
                    frame_type: step.input_type.0,
                    payload: val.clone(),
                }]
            }
        }
        InputBinding::StepOutput(target_step_id) => step_outputs
            .get(target_step_id)
            .cloned()
            .unwrap_or_default(),
    }
}

fn check_frame_limits(step: &ComposedStep, frames: &[FrameEnvelope]) -> Result<(), PipelineError> {
    let frame_count = frames.len() as u64;
    if frame_count > MAX_FRAME_COUNT {
        return Err(resource_exceeded(step, "frame_count", frame_count));
    }
    for frame in frames {
        let size = serde_json::to_vec(&frame.payload)
            .map(|v| v.len() as u64)
            .unwrap_or(0);
        if size > MAX_FRAME_SIZE {
            return Err(resource_exceeded(step, "frame_size", size));
        }
    }
    Ok(())
}

fn check_step_limits(
    step: &ComposedStep,
    quota: &ResourceQuota,
    elapsed_ms: u64,
    ctx: &ExecutionContext,
) -> Result<(), PipelineError> {
    if elapsed_ms > quota.cpu_ms {
        return Err(resource_exceeded(step, "cpu_ms", elapsed_ms));
    }
    if ctx.resource_used.memory_mb > quota.memory_mb {
        return Err(resource_exceeded(
            step,
            "memory_mb",
            ctx.resource_used.memory_mb,
        ));
    }
    Ok(())
}

fn resource_exceeded(step: &ComposedStep, limit: &str, current: u64) -> PipelineError {
    PipelineError {
        retryable: false,
        failed_step_id: step.step_id.clone(),
        error_kind: ErrorKind::ResourceExceeded {
            limit: limit.to_string(),
            current,
        },
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Execute a single composed step with full policy and resource checks.
fn execute_step(
    step: &ComposedStep,
    input_frames: Vec<FrameEnvelope>,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
) -> Result<Vec<FrameEnvelope>, PipelineError> {
    let manifest = registry
        .inspect(&step.command_id)
        .map_err(|_| PipelineError {
            retryable: false,
            failed_step_id: step.step_id.clone(),
            error_kind: ErrorKind::CommandNotFound,
        })?;

    if let Some(quota) = &manifest.resource_quota {
        check_frame_limits(step, &input_frames)?;
        check_step_limits(step, quota, 0, ctx)?;
    }

    let policy_decision = policy.check(step, ctx);
    match policy_decision {
        PolicyDecision::Deny => {
            return Err(PipelineError {
                retryable: false,
                failed_step_id: step.step_id.clone(),
                error_kind: ErrorKind::SideEffectBlocked,
            });
        }
        PolicyDecision::RequireApproval => {
            return Err(PipelineError {
                retryable: true,
                failed_step_id: step.step_id.clone(),
                error_kind: ErrorKind::ApprovalRequired,
            });
        }
        PolicyDecision::Allow => {}
    }

    let executor = executors
        .get(&step.command_id)
        .ok_or_else(|| PipelineError {
            retryable: false,
            failed_step_id: step.step_id.clone(),
            error_kind: ErrorKind::CommandNotFound,
        })?;

    let start = Instant::now();
    let result = executor.execute(step, input_frames, ctx);
    let elapsed_ms = start.elapsed().as_millis() as u64;
    ctx.resource_used.cpu_ms += elapsed_ms;

    if let Some(quota) = &manifest.resource_quota {
        check_step_limits(step, quota, elapsed_ms, ctx)?;
    }

    let mut output_frames = match result {
        Ok(frames) => {
            for frame in &frames {
                if frame.frame_type != step.output_type.0 {
                    return Err(PipelineError {
                        retryable: false,
                        failed_step_id: step.step_id.clone(),
                        error_kind: ErrorKind::TypeMismatch,
                    });
                }
            }
            if let Some(_quota) = &manifest.resource_quota {
                check_frame_limits(step, &frames)?;
            }
            ctx.resource_used.frame_count += frames.len() as u64;
            frames
        }
        Err(err) => match &step.failure_policy {
            crate::ast::FailurePolicy::FailFast => return Err(err),
            crate::ast::FailurePolicy::Skip => {
                ctx.audit.push(AuditRecord {
                    step_id: step.step_id.clone(),
                    command_id: step.command_id.clone(),
                    decision: format!("skipped: {:?}", err.error_kind),
                    timestamp_ms: now_ms(),
                });
                Vec::new()
            }
            crate::ast::FailurePolicy::Default(val) => vec![FrameEnvelope {
                seq: 0,
                step_id: step.step_id.clone(),
                frame_type: step.output_type.0,
                payload: val.clone(),
            }],
        },
    };

    for (i, frame) in output_frames.iter_mut().enumerate() {
        frame.seq = i as u64;
    }

    if !ctx.audit.iter().any(|a| a.step_id == step.step_id) {
        ctx.audit.push(AuditRecord {
            step_id: step.step_id.clone(),
            command_id: step.command_id.clone(),
            decision: match policy_decision {
                PolicyDecision::Allow => "allow".to_string(),
                PolicyDecision::Deny => "deny".to_string(),
                PolicyDecision::RequireApproval => "approval_required".to_string(),
            },
            timestamp_ms: now_ms(),
        });
    }

    Ok(output_frames)
}

// ============================================================================
// DAG runtime
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Completed,
    Skipped,
}

// ============================================================================
// Checkpoint & Resume
// ============================================================================

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

fn execute_step_with_retry(
    step: &ComposedStep,
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

fn save_checkpoint(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationRecord {
    pub step_id: String,
    pub success: bool,
    pub error: Option<String>,
}

pub trait CompensatingExecutor: CommandExecutor {
    fn compensate(
        &self,
        step: &ComposedStep,
        executed_output: &[FrameEnvelope],
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError>;
}

// ============================================================================
// DAG runtime
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn execute_dag_core(
    dag: &DagPipeline,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
    checkpoint_store: Option<&dyn CheckpointStore>,
    retry_policy: Option<&RetryPolicy>,
    step_outputs: &mut HashMap<String, Vec<FrameEnvelope>>,
    step_states: &mut HashMap<String, StepState>,
    branch_selections: &mut HashMap<String, String>,
) -> Result<(), PipelineError> {
    let initial = FrameEnvelope {
        seq: 0,
        step_id: "input".to_string(),
        frame_type: dag.input_type.0,
        payload: initial_input,
    };

    let topo_order: HashMap<String, usize> = dag
        .steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.composed.step_id.clone(), i))
        .collect();

    for step in &dag.steps {
        if step_states.contains_key(&step.composed.step_id) {
            if matches!(step.kind, DagNodeKind::Branch) {
                let input = resolve_dag_input(
                    step,
                    step_outputs,
                    step_states,
                    branch_selections,
                    &initial,
                    &topo_order,
                );
                if let Some(target) = evaluate_branch(step, &input) {
                    branch_selections.insert(step.composed.step_id.clone(), target);
                }
            }
            continue;
        }

        if should_skip_step(step, branch_selections) {
            step_states.insert(step.composed.step_id.clone(), StepState::Skipped);
            step_outputs.insert(step.composed.step_id.clone(), Vec::new());
            if let Some(store) = checkpoint_store {
                save_checkpoint(store, dag, step, &[], StepState::Skipped, ctx)?;
            }
            continue;
        }

        if matches!(step.kind, DagNodeKind::Exit) {
            let input = resolve_dag_input(
                step,
                step_outputs,
                step_states,
                branch_selections,
                &initial,
                &topo_order,
            );
            step_outputs.insert(step.composed.step_id.clone(), input.clone());
            step_states.insert(step.composed.step_id.clone(), StepState::Completed);
            if let Some(store) = checkpoint_store {
                save_checkpoint(store, dag, step, &input, StepState::Completed, ctx)?;
            }
            continue;
        }

        let input_frames = resolve_dag_input(
            step,
            step_outputs,
            step_states,
            branch_selections,
            &initial,
            &topo_order,
        );

        let output_frames = match &step.kind {
            DagNodeKind::Command => {
                if let Some(rp) = retry_policy {
                    execute_step_with_retry(
                        &step.composed,
                        input_frames,
                        registry,
                        executors,
                        policy,
                        ctx,
                        rp,
                    )?
                } else {
                    execute_step(
                        &step.composed,
                        input_frames,
                        registry,
                        executors,
                        policy,
                        ctx,
                    )?
                }
            }
            DagNodeKind::Tee => input_frames,
            DagNodeKind::Branch => {
                let selected = evaluate_branch(step, &input_frames);
                if let Some(target) = selected {
                    branch_selections.insert(step.composed.step_id.clone(), target);
                }
                input_frames
            }
            DagNodeKind::Join { .. } => input_frames,
            DagNodeKind::Exit => unreachable!(),
        };

        step_outputs.insert(step.composed.step_id.clone(), output_frames.clone());
        step_states.insert(step.composed.step_id.clone(), StepState::Completed);

        if let Some(store) = checkpoint_store {
            save_checkpoint(store, dag, step, &output_frames, StepState::Completed, ctx)?;
        }
    }

    Ok(())
}

/// Run a DAG pipeline with real executors.
///
/// # Errors
/// Returns `PipelineError` if the pipeline is empty, a step fails and the
/// failure policy is `FailFast`, or a resource limit is exceeded.
pub fn run_dag(
    dag: &DagPipeline,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
) -> Result<RunResult, PipelineError> {
    if dag.steps.is_empty() {
        return Err(PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        });
    }

    let mut step_outputs: HashMap<String, Vec<FrameEnvelope>> = HashMap::new();
    let mut step_states: HashMap<String, StepState> = HashMap::new();
    let mut branch_selections: HashMap<String, String> = HashMap::new();

    execute_dag_core(
        dag,
        initial_input,
        registry,
        executors,
        policy,
        ctx,
        None,
        None,
        &mut step_outputs,
        &mut step_states,
        &mut branch_selections,
    )?;

    let exit_outputs: Vec<Vec<FrameEnvelope>> = dag
        .steps
        .iter()
        .filter(|s| matches!(s.kind, DagNodeKind::Exit))
        .filter_map(|s| step_outputs.get(&s.composed.step_id))
        .cloned()
        .collect();

    let final_output = if exit_outputs.is_empty() {
        dag.steps
            .iter()
            .rev()
            .find(|s| !matches!(s.kind, DagNodeKind::Exit))
            .and_then(|s| step_outputs.get(&s.composed.step_id))
            .cloned()
            .unwrap_or_default()
    } else {
        exit_outputs.into_iter().flatten().collect()
    };

    Ok(RunResult {
        output: final_output,
        audit: ctx.audit.clone(),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn run_dag_with_checkpoint(
    dag: &DagPipeline,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
    checkpoint_store: &dyn CheckpointStore,
    retry_policy: Option<&RetryPolicy>,
) -> Result<RunResult, PipelineError> {
    if dag.steps.is_empty() {
        return Err(PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        });
    }

    let mut step_outputs: HashMap<String, Vec<FrameEnvelope>> = HashMap::new();
    let mut step_states: HashMap<String, StepState> = HashMap::new();
    let mut branch_selections: HashMap<String, String> = HashMap::new();

    execute_dag_core(
        dag,
        initial_input,
        registry,
        executors,
        policy,
        ctx,
        Some(checkpoint_store),
        retry_policy,
        &mut step_outputs,
        &mut step_states,
        &mut branch_selections,
    )?;

    let exit_outputs: Vec<Vec<FrameEnvelope>> = dag
        .steps
        .iter()
        .filter(|s| matches!(s.kind, DagNodeKind::Exit))
        .filter_map(|s| step_outputs.get(&s.composed.step_id))
        .cloned()
        .collect();

    let final_output = if exit_outputs.is_empty() {
        dag.steps
            .iter()
            .rev()
            .find(|s| !matches!(s.kind, DagNodeKind::Exit))
            .and_then(|s| step_outputs.get(&s.composed.step_id))
            .cloned()
            .unwrap_or_default()
    } else {
        exit_outputs.into_iter().flatten().collect()
    };

    Ok(RunResult {
        output: final_output,
        audit: ctx.audit.clone(),
    })
}

pub fn resume_dag(
    dag: &DagPipeline,
    checkpoint_store: &dyn CheckpointStore,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
) -> Result<RunResult, PipelineError> {
    resume_dag_with_retry(
        dag,
        checkpoint_store,
        initial_input,
        registry,
        executors,
        policy,
        ctx,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn resume_dag_with_retry(
    dag: &DagPipeline,
    checkpoint_store: &dyn CheckpointStore,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    ctx: &mut ExecutionContext,
    retry_policy: Option<&RetryPolicy>,
) -> Result<RunResult, PipelineError> {
    if dag.steps.is_empty() {
        return Err(PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        });
    }

    let mut step_outputs: HashMap<String, Vec<FrameEnvelope>> = HashMap::new();
    let mut step_states: HashMap<String, StepState> = HashMap::new();
    let mut branch_selections: HashMap<String, String> = HashMap::new();

    let checkpoints = checkpoint_store
        .load_all(&dag.pipeline_id)
        .map_err(|_| PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        })?;

    if let Some(latest) = checkpoint_store
        .load_latest(&dag.pipeline_id)
        .map_err(|_| PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        })?
    {
        ctx.resource_used = latest.resource_used;
    }

    for cp in &checkpoints {
        step_outputs.insert(cp.step_id.clone(), cp.output_frames.clone());
        step_states.insert(cp.step_id.clone(), cp.step_state);
    }

    execute_dag_core(
        dag,
        initial_input,
        registry,
        executors,
        policy,
        ctx,
        Some(checkpoint_store),
        retry_policy,
        &mut step_outputs,
        &mut step_states,
        &mut branch_selections,
    )?;

    let exit_outputs: Vec<Vec<FrameEnvelope>> = dag
        .steps
        .iter()
        .filter(|s| matches!(s.kind, DagNodeKind::Exit))
        .filter_map(|s| step_outputs.get(&s.composed.step_id))
        .cloned()
        .collect();

    let final_output = if exit_outputs.is_empty() {
        dag.steps
            .iter()
            .rev()
            .find(|s| !matches!(s.kind, DagNodeKind::Exit))
            .and_then(|s| step_outputs.get(&s.composed.step_id))
            .cloned()
            .unwrap_or_default()
    } else {
        exit_outputs.into_iter().flatten().collect()
    };

    Ok(RunResult {
        output: final_output,
        audit: ctx.audit.clone(),
    })
}

pub fn rollback_dag(
    dag: &DagPipeline,
    checkpoint_store: &dyn CheckpointStore,
    _registry: &CommandRegistry,
    compensators: &HashMap<String, Box<dyn CompensatingExecutor>>,
    ctx: &mut ExecutionContext,
) -> Result<Vec<CompensationRecord>, PipelineError> {
    let checkpoints = checkpoint_store
        .load_all(&dag.pipeline_id)
        .map_err(|_| PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        })?;

    let checkpoint_ids: std::collections::HashSet<String> =
        checkpoints.iter().map(|c| c.step_id.clone()).collect();

    let mut records = Vec::new();

    for step in dag.steps.iter().rev() {
        if !checkpoint_ids.contains(&step.composed.step_id) {
            continue;
        }

        let checkpoint = checkpoints
            .iter()
            .find(|c| c.step_id == step.composed.step_id)
            .expect("checkpoint exists for step in set");

        if checkpoint.step_state != StepState::Completed {
            continue;
        }

        if let Some(compensator) = compensators.get(&step.composed.command_id) {
            let record =
                match compensator.compensate(&step.composed, &checkpoint.output_frames, ctx) {
                    Ok(_) => CompensationRecord {
                        step_id: step.composed.step_id.clone(),
                        success: true,
                        error: None,
                    },
                    Err(err) => CompensationRecord {
                        step_id: step.composed.step_id.clone(),
                        success: false,
                        error: Some(err.error_kind.to_string()),
                    },
                };
            records.push(record);
        }
    }

    Ok(records)
}

fn should_skip_step(step: &DagStep, branch_selections: &HashMap<String, String>) -> bool {
    for pred_id in &step.predecessors {
        if let Some(selected) = branch_selections.get(pred_id) {
            if *selected != step.composed.step_id {
                return true;
            }
        }
    }
    false
}

fn evaluate_branch(step: &DagStep, input_frames: &[FrameEnvelope]) -> Option<String> {
    let branches = step.composed.args.get("branches")?;
    let branches_arr = branches.as_array()?;

    if let Some(route) = step.composed.args.get("route").and_then(|v| v.as_str()) {
        for branch in branches_arr {
            if let Some(target) = branch.get("target").and_then(|v| v.as_str()) {
                if let Some(name) = branch.get("name").and_then(|v| v.as_str()) {
                    if name == route {
                        return Some(target.to_string());
                    }
                }
            }
        }
    }

    if let Some(frame) = input_frames.first() {
        for branch in branches_arr {
            if let (Some(target), Some(predicate)) = (
                branch.get("target").and_then(|v| v.as_str()),
                branch.get("predicate"),
            ) {
                if &frame.payload == predicate {
                    return Some(target.to_string());
                }
                if let (Some(payload_obj), Some(pred_obj)) =
                    (frame.payload.as_object(), predicate.as_object())
                {
                    if pred_obj.iter().all(|(k, v)| payload_obj.get(k) == Some(v)) {
                        return Some(target.to_string());
                    }
                }
            }
        }
    }

    branches_arr
        .first()
        .and_then(|b| b.get("target"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

fn merge_inputs(
    inputs: Vec<(usize, Vec<FrameEnvelope>)>,
    strategy: &DagMergeStrategy,
) -> Vec<FrameEnvelope> {
    match strategy {
        DagMergeStrategy::Concat => inputs
            .into_iter()
            .flat_map(|(_, frames)| frames)
            .enumerate()
            .map(|(i, mut f)| {
                f.seq = i as u64;
                f
            })
            .collect(),
        DagMergeStrategy::Interleave => {
            let mut result = Vec::new();
            let mut idx = 0u64;
            let max_len = inputs.iter().map(|(_, f)| f.len()).max().unwrap_or(0);
            for i in 0..max_len {
                for (_, frames) in &inputs {
                    if i < frames.len() {
                        let mut f = frames[i].clone();
                        f.seq = idx;
                        result.push(f);
                        idx += 1;
                    }
                }
            }
            result
        }
        DagMergeStrategy::FirstNonEmpty => {
            for (_, frames) in inputs {
                if !frames.is_empty() {
                    return frames
                        .into_iter()
                        .enumerate()
                        .map(|(i, mut f)| {
                            f.seq = i as u64;
                            f
                        })
                        .collect();
                }
            }
            Vec::new()
        }
    }
}

fn resolve_dag_input(
    step: &DagStep,
    step_outputs: &HashMap<String, Vec<FrameEnvelope>>,
    _step_states: &HashMap<String, StepState>,
    branch_selections: &HashMap<String, String>,
    initial: &FrameEnvelope,
    topo_order: &HashMap<String, usize>,
) -> Vec<FrameEnvelope> {
    let mut inputs: Vec<(usize, Vec<FrameEnvelope>)> = Vec::new();
    let mut has_real_predecessor = false;

    for pred_id in &step.predecessors {
        if let Some(selected) = branch_selections.get(pred_id) {
            if *selected != step.composed.step_id {
                continue;
            }
        }

        let frames = if let Some(frames) = step_outputs.get(pred_id) {
            has_real_predecessor = true;
            frames.clone()
        } else {
            vec![initial.clone()]
        };

        let order = topo_order.get(pred_id).copied().unwrap_or(0);
        inputs.push((order, frames));
    }

    if inputs.is_empty() || !has_real_predecessor {
        return expand_initial(step, initial);
    }

    inputs.sort_by_key(|(order, _)| *order);

    match &step.kind {
        DagNodeKind::Join { strategy } => merge_inputs(inputs, strategy),
        _ => {
            if inputs.len() == 1 {
                inputs[0].1.clone()
            } else {
                inputs.into_iter().flat_map(|(_, frames)| frames).collect()
            }
        }
    }
}

fn expand_initial(step: &DagStep, initial: &FrameEnvelope) -> Vec<FrameEnvelope> {
    let val = &initial.payload;
    if step.composed.input_type.1 == Cardinality::Many && val.is_array() {
        val.as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, v)| FrameEnvelope {
                seq: i as u64,
                step_id: step.composed.step_id.clone(),
                frame_type: step.composed.input_type.0,
                payload: v.clone(),
            })
            .collect()
    } else {
        vec![FrameEnvelope {
            seq: 0,
            step_id: step.composed.step_id.clone(),
            frame_type: step.composed.input_type.0,
            payload: val.clone(),
        }]
    }
}

// ============================================================================
// Built-in executors
// ============================================================================

/// Stub executor for `wiki.search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WikiSearchExecutor;

impl CommandExecutor for WikiSearchExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![
            FrameEnvelope {
                seq: 1,
                step_id: step.step_id.clone(),
                frame_type: FrameType::ParagraphFrame,
                payload: serde_json::json!({
                    "paragraph_id": "para-1",
                    "text": "simulated paragraph",
                    "_simulated": true,
                    "_command": "wiki.search"
                }),
            },
            FrameEnvelope {
                seq: 2,
                step_id: step.step_id.clone(),
                frame_type: FrameType::ParagraphFrame,
                payload: serde_json::json!({
                    "paragraph_id": "para-2",
                    "text": "simulated paragraph",
                    "_simulated": true,
                    "_command": "wiki.search"
                }),
            },
        ])
    }
}

/// Stub executor for `citation.resolve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CitationResolveExecutor;

impl CommandExecutor for CitationResolveExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![
            FrameEnvelope {
                seq: 1,
                step_id: step.step_id.clone(),
                frame_type: FrameType::CitedParagraph,
                payload: serde_json::json!({
                    "citation_id": "cite-1",
                    "text": "simulated cited paragraph",
                    "_simulated": true,
                    "_command": "citation.resolve"
                }),
            },
            FrameEnvelope {
                seq: 2,
                step_id: step.step_id.clone(),
                frame_type: FrameType::CitedParagraph,
                payload: serde_json::json!({
                    "citation_id": "cite-2",
                    "text": "simulated cited paragraph",
                    "_simulated": true,
                    "_command": "citation.resolve"
                }),
            },
        ])
    }
}

/// Stub executor for `adr.summarize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdrSummarizeExecutor;

impl CommandExecutor for AdrSummarizeExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![FrameEnvelope {
            seq: 1,
            step_id: step.step_id.clone(),
            frame_type: FrameType::TextAnswer,
            payload: serde_json::json!({
                "text": "simulated answer",
                "citations": [],
                "_simulated": true,
                "_command": "adr.summarize"
            }),
        }])
    }
}

/// Stub executor for `wiki.patch.propose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WikiPatchProposeExecutor;

impl CommandExecutor for WikiPatchProposeExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![FrameEnvelope {
            seq: 1,
            step_id: step.step_id.clone(),
            frame_type: FrameType::PatchProposalArtifact,
            payload: serde_json::json!({
                "patch_id": format!("patch-{}", step.step_id),
                "diff": "simulated diff",
                "_simulated": true,
                "_command": "wiki.patch.propose"
            }),
        }])
    }
}

/// Real executor for `filter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterExecutor;

impl CommandExecutor for FilterExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        let predicate = step
            .args
            .get("predicate")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let filtered: Vec<FrameEnvelope> = input
            .into_iter()
            .filter(|frame| match &predicate {
                serde_json::Value::String(s) => frame.payload.to_string().contains(s),
                serde_json::Value::Object(pred_obj) => {
                    if let Some(frame_obj) = frame.payload.as_object() {
                        pred_obj.iter().all(|(k, v)| frame_obj.get(k) == Some(v))
                    } else {
                        false
                    }
                }
                _ => true,
            })
            .collect();

        Ok(filtered)
    }
}

/// Real executor for `map`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapExecutor;

impl CommandExecutor for MapExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        let transform = step
            .args
            .get("transform")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let mapped: Vec<FrameEnvelope> = input
            .into_iter()
            .map(|mut frame| {
                if let (Some(frame_obj), Some(trans_obj)) =
                    (frame.payload.as_object_mut(), transform.as_object())
                {
                    for (k, v) in trans_obj {
                        frame_obj.insert(k.clone(), v.clone());
                    }
                }
                frame.step_id = step.step_id.clone();
                frame.frame_type = step.output_type.0;
                frame
            })
            .collect();

        Ok(mapped)
    }
}
