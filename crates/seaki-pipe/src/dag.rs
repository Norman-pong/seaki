//! DAG pipeline execution runtime.

use std::collections::HashMap;

use crate::approval_gate::{ApprovalGate, ApprovalRequestInput};
use crate::ast::{Cardinality, DagMergeStrategy, DagNodeKind, DagPipeline, DagStep};
use crate::checkpoint::{
    execute_step_with_retry, execute_step_with_retry_unchecked, save_checkpoint, CheckpointStore,
    RetryPolicy,
};
use crate::compensate::rollback_dag;
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::registry::CommandRegistry;
use crate::run::{
    execute_step, execute_step_core, CommandExecutor, CompensatingExecutor, ExecutionContext,
    RunResult, StepPolicy, StepState,
};
use crate::state_machine::{PipelineState, PipelineStateMachine, StateEvent};
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
    approval_gate: Option<&dyn ApprovalGate>,
    mut state_machine: Option<&mut PipelineStateMachine>,
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

    if let Some(sm) = state_machine.as_mut() {
        sm.transition(StateEvent::Start)?;
    }

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

        let step_result = match &step.kind {
            DagNodeKind::Command => {
                if let Some(sm) = state_machine.as_mut() {
                    sm.transition(StateEvent::StepStarted {
                        step_id: step.composed.step_id.clone(),
                    })?;
                }

                if let Some(ag) = approval_gate {
                    let decision = policy.check(&step.composed, ctx);
                    match decision {
                        crate::run::PolicyDecision::Deny => {
                            return Err(PipelineError {
                                retryable: false,
                                failed_step_id: step.composed.step_id.clone(),
                                error_kind: ErrorKind::SideEffectBlocked,
                            });
                        }
                        crate::run::PolicyDecision::RequireApproval => {
                            let approval_id = ag.request_approval(ApprovalRequestInput {
                                pipeline_id: ctx.pipeline_id.clone(),
                                step_id: step.composed.step_id.clone(),
                                actor_id: ctx.actor_id.clone(),
                                workspace_id: ctx.workspace_id.clone(),
                                operation: step.composed.command_id.clone(),
                                reason: format!("step {} requires approval", step.composed.step_id),
                            })?;
                            if let Some(sm) = state_machine.as_mut() {
                                sm.transition(StateEvent::ApprovalRequested {
                                    approval_id: approval_id.clone(),
                                })?;
                            }
                            let timeout = 30_000u64;
                            match ag.wait_for_approval(&approval_id, timeout)? {
                                seaki_policy::ApprovalStatus::Approved => {
                                    if let Some(sm) = state_machine.as_mut() {
                                        sm.transition(StateEvent::ApprovalGranted)?;
                                    }
                                    if let Some(rp) = retry_policy {
                                        execute_step_with_retry_unchecked(
                                            &step.composed,
                                            input_frames,
                                            registry,
                                            executors,
                                            ctx,
                                            rp,
                                        )
                                    } else {
                                        execute_step_core(
                                            &step.composed,
                                            input_frames,
                                            registry,
                                            executors,
                                            ctx,
                                            crate::run::PolicyDecision::Allow,
                                        )
                                    }
                                }
                                seaki_policy::ApprovalStatus::Denied => {
                                    if let Some(sm) = state_machine.as_mut() {
                                        sm.transition(StateEvent::ApprovalDenied)?;
                                    }
                                    Err(PipelineError {
                                        retryable: false,
                                        failed_step_id: step.composed.step_id.clone(),
                                        error_kind: ErrorKind::ApprovalRequired,
                                    })
                                }
                                seaki_policy::ApprovalStatus::Pending => {
                                    if let Some(sm) = state_machine.as_mut() {
                                        sm.transition(StateEvent::ApprovalTimeout)?;
                                    }
                                    Err(PipelineError {
                                        retryable: false,
                                        failed_step_id: step.composed.step_id.clone(),
                                        error_kind: ErrorKind::ApprovalRequired,
                                    })
                                }
                            }
                        }
                        crate::run::PolicyDecision::Allow => {
                            if let Some(rp) = retry_policy {
                                execute_step_with_retry(
                                    &step.composed,
                                    input_frames,
                                    registry,
                                    executors,
                                    policy,
                                    ctx,
                                    rp,
                                )
                            } else {
                                execute_step(
                                    &step.composed,
                                    input_frames,
                                    registry,
                                    executors,
                                    policy,
                                    ctx,
                                )
                            }
                        }
                    }
                } else if let Some(rp) = retry_policy {
                    execute_step_with_retry(
                        &step.composed,
                        input_frames,
                        registry,
                        executors,
                        policy,
                        ctx,
                        rp,
                    )
                } else {
                    execute_step(
                        &step.composed,
                        input_frames,
                        registry,
                        executors,
                        policy,
                        ctx,
                    )
                }
            }
            DagNodeKind::Tee => Ok(input_frames),
            DagNodeKind::Branch => {
                let selected = evaluate_branch(step, &input_frames);
                if let Some(target) = selected {
                    branch_selections.insert(step.composed.step_id.clone(), target);
                }
                Ok(input_frames)
            }
            DagNodeKind::Join { .. } => Ok(input_frames),
            DagNodeKind::Exit => unreachable!(),
        };

        let output_frames = match step_result {
            Ok(frames) => frames,
            Err(err) => {
                if let Some(sm) = state_machine.as_mut() {
                    let is_retry_exhausted = retry_policy.is_some() && err.retryable;
                    let _ = sm.transition(StateEvent::StepFailed {
                        step_id: step.composed.step_id.clone(),
                        retryable: !is_retry_exhausted && err.retryable,
                    });
                }
                return Err(err);
            }
        };

        step_outputs.insert(step.composed.step_id.clone(), output_frames.clone());
        step_states.insert(step.composed.step_id.clone(), StepState::Completed);

        if let Some(sm) = state_machine.as_mut() {
            sm.transition(StateEvent::StepCompleted {
                step_id: step.composed.step_id.clone(),
            })?;
        }

        if let Some(store) = checkpoint_store {
            save_checkpoint(store, dag, step, &output_frames, StepState::Completed, ctx)?;
        }
    }

    if let Some(sm) = state_machine.as_mut() {
        sm.transition(StateEvent::Complete)?;
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
        None,
        None,
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
        None,
        None,
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
        None,
        None,
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
pub fn run_dag_with_approval(
    dag: &DagPipeline,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn StepPolicy,
    approval_gate: &dyn ApprovalGate,
    checkpoint_store: &dyn CheckpointStore,
    compensators: &HashMap<String, Box<dyn CompensatingExecutor>>,
    ctx: &mut ExecutionContext,
    state_machine: &mut PipelineStateMachine,
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

    let result = execute_dag_core(
        dag,
        initial_input,
        registry,
        executors,
        policy,
        ctx,
        Some(checkpoint_store),
        None,
        &mut step_outputs,
        &mut step_states,
        &mut branch_selections,
        Some(approval_gate),
        Some(state_machine),
    );

    if let Err(ref _err) = result {
        if matches!(
            state_machine.state,
            PipelineState::Failed | PipelineState::Cancelled
        ) {
            let _ = rollback_dag(dag, checkpoint_store, registry, compensators, ctx);
            let _ = state_machine.transition(StateEvent::CompensateCompleted);
        }
    }

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

    result.map(|_| RunResult {
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
