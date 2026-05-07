//! Typed pipeline AST: compose, type checking, cardinality.

use crate::registry::{CommandRegistry, SideEffectLevel};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FrameType {
    ParagraphFrame,
    CitedParagraph,
    TextAnswer,
    PatchProposalArtifact,
    JsonValue,
}

impl std::fmt::Display for FrameType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::ParagraphFrame => "ParagraphFrame",
            Self::CitedParagraph => "CitedParagraph",
            Self::TextAnswer => "TextAnswer",
            Self::PatchProposalArtifact => "PatchProposalArtifact",
            Self::JsonValue => "JsonValue",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cardinality {
    One,
    Many,
}

impl std::fmt::Display for Cardinality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::One => f.write_str("One"),
            Self::Many => f.write_str("Many"),
        }
    }
}

pub type TypedFrame = (FrameType, Cardinality);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputBinding {
    PreviousStep,
    Constant(serde_json::Value),
    StepOutput(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailurePolicy {
    FailFast,
    Skip,
    Default(serde_json::Value),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineStep {
    pub step_id: String,
    pub command_id: String,
    pub input_binding: InputBinding,
    pub failure_policy: FailurePolicy,
    #[serde(default = "default_args")]
    pub args: serde_json::Value,
}

fn default_args() -> serde_json::Value {
    serde_json::json!({})
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineAst {
    pub pipeline_id: String,
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedStep {
    pub step_id: String,
    pub command_id: String,
    pub input_type: TypedFrame,
    pub output_type: TypedFrame,
    pub input_binding: InputBinding,
    pub failure_policy: FailurePolicy,
    pub side_effect_level: SideEffectLevel,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedPipeline {
    pub pipeline_id: String,
    pub steps: Vec<ComposedStep>,
    pub input_type: TypedFrame,
    pub output_type: TypedFrame,
}

/// Merge strategy for Join nodes in a DAG pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DagMergeStrategy {
    Concat,
    Interleave,
    FirstNonEmpty,
}

/// Kind of a node in a DAG pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DagNodeKind {
    Command,
    Tee,
    Branch,
    Join { strategy: DagMergeStrategy },
    Exit,
}

/// A step in a DAG pipeline with topology metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagStep {
    pub composed: ComposedStep,
    pub kind: DagNodeKind,
    pub predecessors: Vec<String>,
    pub successors: Vec<String>,
}

/// A DAG pipeline with topologically sorted steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DagPipeline {
    pub pipeline_id: String,
    pub steps: Vec<DagStep>,
    pub input_type: TypedFrame,
    pub output_type: TypedFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposeError {
    TypeMismatch {
        step_id: String,
        expected: TypedFrame,
        found: TypedFrame,
    },
    CardinalityConflict {
        step_id: String,
        expected: Cardinality,
        found: Cardinality,
    },
    CycleDetected,
    CommandNotFound(String),
    EmptyPipeline,
    StepNotFound(String),
}

impl std::fmt::Display for ComposeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TypeMismatch {
                step_id,
                expected,
                found,
            } => write!(
                f,
                "type mismatch at step {step_id}: expected {expected:?}, found {found:?}"
            ),
            Self::CardinalityConflict {
                step_id,
                expected,
                found,
            } => write!(
                f,
                "cardinality conflict at step {step_id}: expected {expected}, found {found}"
            ),
            Self::CycleDetected => write!(f, "cycle detected in pipeline"),
            Self::CommandNotFound(id) => write!(f, "command not found: {id}"),
            Self::EmptyPipeline => write!(f, "pipeline has no steps"),
            Self::StepNotFound(id) => write!(f, "step not found: {id}"),
        }
    }
}

impl std::error::Error for ComposeError {}

fn dfs<'a>(
    node: &'a str,
    graph: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    stack: &mut HashSet<&'a str>,
) -> Result<(), ComposeError> {
    visited.insert(node);
    stack.insert(node);

    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                dfs(neighbor, graph, visited, stack)?;
            } else if stack.contains(neighbor) {
                return Err(ComposeError::CycleDetected);
            }
        }
    }

    stack.remove(node);
    Ok(())
}

/// Compose a pipeline AST into a validated `ComposedPipeline`.
///
/// # Errors
/// Returns `ComposeError` if the pipeline is empty, contains cycles, or has type mismatches.
pub fn compose(
    ast: &PipelineAst,
    registry: &CommandRegistry,
) -> Result<ComposedPipeline, ComposeError> {
    if ast.steps.is_empty() {
        return Err(ComposeError::EmptyPipeline);
    }

    // Detect cycles when StepOutput references are used.
    detect_cycle(ast)?;

    let mut composed_steps: Vec<ComposedStep> = Vec::with_capacity(ast.steps.len());
    let mut previous_output: Option<TypedFrame> = None;

    for step in &ast.steps {
        let manifest = registry
            .inspect(&step.command_id)
            .map_err(|_| ComposeError::CommandNotFound(step.command_id.clone()))?;

        let expected_input = manifest.input_frame;
        let output_type = manifest.output_frame;

        // Determine the actual input type for this step based on binding.
        let actual_input = match &step.input_binding {
            InputBinding::PreviousStep => previous_output.ok_or(ComposeError::EmptyPipeline)?,
            InputBinding::Constant(_) => expected_input,
            InputBinding::StepOutput(ref target_step_id) => {
                let target = composed_steps
                    .iter()
                    .find(|s| s.step_id == *target_step_id)
                    .ok_or_else(|| ComposeError::StepNotFound(target_step_id.clone()))?;
                target.output_type
            }
        };

        // Type match: frame type must be identical.
        if actual_input.0 != expected_input.0 {
            return Err(ComposeError::TypeMismatch {
                step_id: step.step_id.clone(),
                expected: expected_input,
                found: actual_input,
            });
        }

        // Cardinality compatibility: Many cannot feed into One.
        if actual_input.1 == Cardinality::Many && expected_input.1 == Cardinality::One {
            return Err(ComposeError::CardinalityConflict {
                step_id: step.step_id.clone(),
                expected: Cardinality::One,
                found: Cardinality::Many,
            });
        }

        composed_steps.push(ComposedStep {
            step_id: step.step_id.clone(),
            command_id: step.command_id.clone(),
            input_type: expected_input,
            output_type,
            input_binding: step.input_binding.clone(),
            failure_policy: step.failure_policy.clone(),
            side_effect_level: manifest.side_effect_level,
            args: step.args.clone(),
        });

        previous_output = Some(output_type);
    }

    let input_type = composed_steps
        .first()
        .map_or((FrameType::JsonValue, Cardinality::One), |s| s.input_type);
    let output_type = composed_steps
        .last()
        .map_or((FrameType::JsonValue, Cardinality::One), |s| s.output_type);

    Ok(ComposedPipeline {
        pipeline_id: ast.pipeline_id.clone(),
        steps: composed_steps,
        input_type,
        output_type,
    })
}

fn detect_cycle(ast: &PipelineAst) -> Result<(), ComposeError> {
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    let step_ids: HashSet<&str> = ast.steps.iter().map(|s| s.step_id.as_str()).collect();

    for step in &ast.steps {
        if let InputBinding::StepOutput(ref target) = step.input_binding {
            if !step_ids.contains(target.as_str()) {
                return Err(ComposeError::StepNotFound(target.clone()));
            }
            graph
                .entry(step.step_id.as_str())
                .or_default()
                .push(target.as_str());
        }
    }

    let mut visited = HashSet::new();
    let mut stack = HashSet::new();

    for step_id in &step_ids {
        if !visited.contains(step_id) {
            dfs(step_id, &graph, &mut visited, &mut stack)?;
        }
    }

    Ok(())
}
