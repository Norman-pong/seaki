//! Compensation and rollback support.

use std::collections::HashMap;

use crate::ast::DagPipeline;
use crate::checkpoint::CheckpointStore;
use crate::dry_run::PipelineError;
use crate::registry::CommandRegistry;
use crate::run::StepState;
use crate::run::{CompensatingExecutor, ExecutionContext};
use crate::ErrorKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompensationRecord {
    pub step_id: String,
    pub success: bool,
    pub error: Option<String>,
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
