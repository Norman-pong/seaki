pub mod approval_gate;
pub mod ast;
pub mod checkpoint;
pub mod compensate;
pub mod dag;
pub mod dry_run;
pub mod event;
pub mod executor;
pub mod registry;
pub mod run;
pub mod state_machine;

use std::collections::HashMap;

use crate::ast::{
    compose, Cardinality, ComposedPipeline, ComposedStep, DagMergeStrategy, DagNodeKind,
    DagPipeline, DagStep, FailurePolicy, FrameType, InputBinding, PipelineAst, PipelineStep,
    TypedFrame,
};
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::registry::{CommandRegistry, PipeCommandManifest, SideEffectLevel};
use crate::run::*;
use crate::ErrorKind;

pub fn test_context() -> ExecutionContext {
    ExecutionContext {
        workspace_id: "ws-1".to_string(),
        actor_id: "actor-1".to_string(),
        pipeline_id: "pipe-1".to_string(),
        audit: Vec::new(),
        resource_used: ResourceUsage::default(),
    }
}

pub fn builtin_executors() -> HashMap<String, Box<dyn CommandExecutor>> {
    let mut m: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    m.insert("wiki.search".to_string(), Box::new(WikiSearchExecutor));
    m.insert(
        "citation.resolve".to_string(),
        Box::new(CitationResolveExecutor),
    );
    m.insert("adr.summarize".to_string(), Box::new(AdrSummarizeExecutor));
    m.insert(
        "wiki.patch.propose".to_string(),
        Box::new(WikiPatchProposeExecutor),
    );
    m.insert("filter".to_string(), Box::new(FilterExecutor));
    m.insert("map".to_string(), Box::new(MapExecutor));
    m
}

pub fn registry_with_failing() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    let input = serde_json::json!({"type": "object"});
    let output = serde_json::json!({"type": "object"});
    let manifest = PipeCommandManifest {
        command_id: "failing".to_string(),
        description: "always fails".to_string(),
        input_schema: input.clone(),
        output_schema: output.clone(),
        input_frame: (FrameType::JsonValue, Cardinality::One),
        output_frame: (FrameType::JsonValue, Cardinality::One),
        side_effect_level: SideEffectLevel::None,
        resource_quota: None,
        schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
    };
    registry.register(manifest).unwrap();
    registry
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailingExecutor;

impl CommandExecutor for FailingExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Err(PipelineError {
            retryable: false,
            failed_step_id: step.step_id.clone(),
            error_kind: ErrorKind::CommandNotFound,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn make_dag_step(
    step_id: &str,
    kind: DagNodeKind,
    command_id: &str,
    args: serde_json::Value,
    input_type: TypedFrame,
    output_type: TypedFrame,
    predecessors: Vec<&str>,
    successors: Vec<&str>,
) -> DagStep {
    DagStep {
        composed: ComposedStep {
            step_id: step_id.to_string(),
            command_id: command_id.to_string(),
            input_type,
            output_type,
            input_binding: InputBinding::PreviousStep,
            failure_policy: FailurePolicy::FailFast,
            side_effect_level: SideEffectLevel::None,
            args,
        },
        kind,
        predecessors: predecessors.into_iter().map(|s| s.to_string()).collect(),
        successors: successors.into_iter().map(|s| s.to_string()).collect(),
    }
}
