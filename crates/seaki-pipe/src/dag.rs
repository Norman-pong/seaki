//! DAG pipeline execution runtime.

use std::collections::HashMap;

use crate::ast::{Cardinality, DagMergeStrategy, DagNodeKind, DagPipeline, DagStep};
use crate::checkpoint::{execute_step_with_retry, save_checkpoint, CheckpointStore, RetryPolicy};
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::registry::CommandRegistry;
use crate::run::{
    execute_step, CommandExecutor, ExecutionContext, RunResult, StepPolicy, StepState,
};
use crate::ErrorKind;

#[allow(clippy::too_many_arguments)]
pub(crate) fn execute_dag_core(
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

pub(crate) fn should_skip_step(
    step: &DagStep,
    branch_selections: &HashMap<String, String>,
) -> bool {
    for pred_id in &step.predecessors {
        if let Some(selected) = branch_selections.get(pred_id) {
            if *selected != step.composed.step_id {
                return true;
            }
        }
    }
    false
}

pub(crate) fn evaluate_branch(step: &DagStep, input_frames: &[FrameEnvelope]) -> Option<String> {
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

pub(crate) fn resolve_dag_input(
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
