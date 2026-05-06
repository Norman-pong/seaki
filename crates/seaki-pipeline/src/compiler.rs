//! Pipeline compiler: type-check and validate a `PipelineGraph` against a `CommandRegistry`.

use crate::graph::{GraphError, PipelineGraph};
use seaki_pipe::registry::{CommandRegistry, PipeCommandManifest, ResourceQuota, SideEffectLevel};
use seaki_pipe::TypedFrame;
use std::collections::HashMap;

/// Result of compiling a pipeline graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileResult {
    pub graph_id: String,
    /// Linear steps for simple pipelines; empty for complex DAGs.
    pub linear_steps: Vec<CompiledStep>,
    /// Estimated input frame type for the entire pipeline.
    pub input_type: TypedFrame,
    /// Estimated output frame type for the entire pipeline.
    pub output_type: TypedFrame,
    /// Side-effect level of the most dangerous step.
    pub max_side_effect: SideEffectLevel,
    /// Schema hashes of all commands used (for reproducibility).
    pub command_schema_hashes: HashMap<String, String>,
}

/// A compiled step with resolved type information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledStep {
    pub step_id: String,
    pub command_id: String,
    pub input_type: TypedFrame,
    pub output_type: TypedFrame,
    pub side_effect_level: SideEffectLevel,
    pub resource_quota: Option<ResourceQuota>,
    pub schema_hash: String,
}

/// Errors that can occur during compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    Graph(GraphError),
    Compose(seaki_pipe::ComposeError),
    SchemaHashMismatch {
        command_id: String,
        expected: String,
        found: String,
    },
    EmptyPipeline,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graph(e) => write!(f, "graph error: {e}"),
            Self::Compose(e) => write!(f, "compose error: {e}"),
            Self::SchemaHashMismatch {
                command_id,
                expected,
                found,
            } => write!(
                f,
                "schema hash mismatch for {command_id}: expected {expected}, found {found}"
            ),
            Self::EmptyPipeline => write!(f, "pipeline has no steps"),
        }
    }
}

impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Graph(e) => Some(e),
            Self::Compose(e) => Some(e),
            _ => None,
        }
    }
}

impl From<GraphError> for CompileError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

impl From<seaki_pipe::ComposeError> for CompileError {
    fn from(e: seaki_pipe::ComposeError) -> Self {
        Self::Compose(e)
    }
}

/// Compile a `PipelineGraph` into a `CompileResult`.
///
/// # Errors
/// Returns `CompileError` if the graph is invalid, contains unknown commands,
/// has schema hash mismatches, or has type/cardinality conflicts.
pub fn compile(
    graph: &PipelineGraph,
    registry: &CommandRegistry,
) -> Result<CompileResult, CompileError> {
    graph.validate()?;

    // For M2-P01, we focus on linear pipelines. DAG support comes in M2-P04.
    let ast = graph.to_linear_ast().map_err(|e| {
        // Non-linear graphs are reported as cycle errors for M2-P01.
        CompileError::Graph(e)
    })?;

    if ast.steps.is_empty() {
        return Err(CompileError::EmptyPipeline);
    }

    // Validate schema hashes before delegating to compose.
    for step in &ast.steps {
        let manifest = registry
            .inspect(&step.command_id)
            .map_err(|_| CompileError::Compose(seaki_pipe::ComposeError::CommandNotFound(step.command_id.clone())))?;
        let expected_hash =
            PipeCommandManifest::compute_schema_hash(&manifest.input_schema, &manifest.output_schema);
        if manifest.schema_hash != expected_hash {
            return Err(CompileError::SchemaHashMismatch {
                command_id: step.command_id.clone(),
                expected: expected_hash,
                found: manifest.schema_hash.clone(),
            });
        }
    }

    // Delegate type-checking to seaki_pipe::compose.
    let composed = seaki_pipe::compose(&ast, registry)?;

    let mut linear_steps: Vec<CompiledStep> = Vec::with_capacity(composed.steps.len());
    let mut max_side_effect = SideEffectLevel::None;
    let mut command_schema_hashes: HashMap<String, String> = HashMap::new();

    for step in &composed.steps {
        let manifest = registry.inspect(&step.command_id).map_err(|_| {
            CompileError::Compose(seaki_pipe::ComposeError::CommandNotFound(
                step.command_id.clone(),
            ))
        })?;

        linear_steps.push(CompiledStep {
            step_id: step.step_id.clone(),
            command_id: step.command_id.clone(),
            input_type: step.input_type,
            output_type: step.output_type,
            side_effect_level: step.side_effect_level,
            resource_quota: manifest.resource_quota.clone(),
            schema_hash: manifest.schema_hash.clone(),
        });

        command_schema_hashes.insert(step.command_id.clone(), manifest.schema_hash.clone());

        if step.side_effect_level as u8 > max_side_effect as u8 {
            max_side_effect = step.side_effect_level;
        }
    }

    Ok(CompileResult {
        graph_id: graph.graph_id.clone(),
        linear_steps,
        input_type: composed.input_type,
        output_type: composed.output_type,
        max_side_effect,
        command_schema_hashes,
    })
}
