//! Pipeline compiler: type-check and validate a `PipelineGraph` against a `CommandRegistry`.

use crate::graph::{GraphError, Node, NodeId, PipelineGraph};
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

/// Compile a `PipelineGraph` into a `DagPipeline`.
///
/// # Errors
/// Returns `CompileError` if the graph is invalid, contains unknown commands,
/// has schema hash mismatches, or has type/cardinality conflicts.
pub fn compile_dag(
    graph: &PipelineGraph,
    registry: &CommandRegistry,
) -> Result<seaki_pipe::DagPipeline, CompileError> {
    graph.validate()?;

    let topo = topological_sort(graph)?;

    let mut steps: Vec<seaki_pipe::DagStep> = Vec::new();
    let mut input_type = (
        seaki_pipe::FrameType::JsonValue,
        seaki_pipe::Cardinality::One,
    );
    let mut output_type = (
        seaki_pipe::FrameType::JsonValue,
        seaki_pipe::Cardinality::One,
    );

    let mut predecessors: HashMap<String, Vec<String>> = HashMap::new();
    let mut successors: HashMap<String, Vec<String>> = HashMap::new();
    for edge in &graph.edges {
        predecessors
            .entry(edge.to.0.clone())
            .or_default()
            .push(edge.from.0.clone());
        successors
            .entry(edge.from.0.clone())
            .or_default()
            .push(edge.to.0.clone());
    }

    for node_id in &topo {
        let node = graph
            .get_node(node_id)
            .ok_or(GraphError::NodeNotFound(node_id.clone()))?;

        if matches!(node, Node::Entry { .. }) {
            continue;
        }

        let preds = predecessors.get(&node_id.0).cloned().unwrap_or_default();
        let succs = successors.get(&node_id.0).cloned().unwrap_or_default();

        let dag_step = match node {
            Node::Command {
                command_id, args, ..
            } => {
                let manifest = registry.inspect(command_id).map_err(|_| {
                    CompileError::Compose(seaki_pipe::ComposeError::CommandNotFound(
                        command_id.clone(),
                    ))
                })?;
                validate_manifest_hash(manifest)?;

                seaki_pipe::DagStep {
                    composed: seaki_pipe::ComposedStep {
                        step_id: node_id.0.clone(),
                        command_id: command_id.clone(),
                        input_type: manifest.input_frame,
                        output_type: manifest.output_frame,
                        input_binding: seaki_pipe::InputBinding::PreviousStep,
                        failure_policy: seaki_pipe::FailurePolicy::FailFast,
                        side_effect_level: manifest.side_effect_level,
                        args: args.clone(),
                    },
                    kind: seaki_pipe::DagNodeKind::Command,
                    predecessors: preds,
                    successors: succs,
                }
            }
            Node::Tee { .. } => seaki_pipe::DagStep {
                composed: seaki_pipe::ComposedStep {
                    step_id: node_id.0.clone(),
                    command_id: String::new(),
                    input_type: (
                        seaki_pipe::FrameType::JsonValue,
                        seaki_pipe::Cardinality::One,
                    ),
                    output_type: (
                        seaki_pipe::FrameType::JsonValue,
                        seaki_pipe::Cardinality::One,
                    ),
                    input_binding: seaki_pipe::InputBinding::PreviousStep,
                    failure_policy: seaki_pipe::FailurePolicy::FailFast,
                    side_effect_level: seaki_pipe::SideEffectLevel::None,
                    args: serde_json::json!({}),
                },
                kind: seaki_pipe::DagNodeKind::Tee,
                predecessors: preds,
                successors: succs,
            },
            Node::Branch {
                condition,
                branches,
                ..
            } => {
                let branch_args = serde_json::json!({
                    "branches": branches.iter().map(|(n, v)| serde_json::json!({
                        "target": n.0,
                        "predicate": v
                    })).collect::<Vec<_>>(),
                    "condition": match condition {
                        crate::graph::BranchCondition::FrameType => "frame_type",
                        crate::graph::BranchCondition::Predicate { field, .. } => field,
                    }
                });
                seaki_pipe::DagStep {
                    composed: seaki_pipe::ComposedStep {
                        step_id: node_id.0.clone(),
                        command_id: String::new(),
                        input_type: (
                            seaki_pipe::FrameType::JsonValue,
                            seaki_pipe::Cardinality::One,
                        ),
                        output_type: (
                            seaki_pipe::FrameType::JsonValue,
                            seaki_pipe::Cardinality::One,
                        ),
                        input_binding: seaki_pipe::InputBinding::PreviousStep,
                        failure_policy: seaki_pipe::FailurePolicy::FailFast,
                        side_effect_level: seaki_pipe::SideEffectLevel::None,
                        args: branch_args,
                    },
                    kind: seaki_pipe::DagNodeKind::Branch,
                    predecessors: preds,
                    successors: succs,
                }
            }
            Node::Join { merge_strategy, .. } => {
                let strategy = match merge_strategy {
                    crate::graph::MergeStrategy::Concat => seaki_pipe::DagMergeStrategy::Concat,
                    crate::graph::MergeStrategy::Interleave => {
                        seaki_pipe::DagMergeStrategy::Interleave
                    }
                    crate::graph::MergeStrategy::FirstNonEmpty => {
                        seaki_pipe::DagMergeStrategy::FirstNonEmpty
                    }
                };
                seaki_pipe::DagStep {
                    composed: seaki_pipe::ComposedStep {
                        step_id: node_id.0.clone(),
                        command_id: String::new(),
                        input_type: (
                            seaki_pipe::FrameType::JsonValue,
                            seaki_pipe::Cardinality::One,
                        ),
                        output_type: (
                            seaki_pipe::FrameType::JsonValue,
                            seaki_pipe::Cardinality::One,
                        ),
                        input_binding: seaki_pipe::InputBinding::PreviousStep,
                        failure_policy: seaki_pipe::FailurePolicy::FailFast,
                        side_effect_level: seaki_pipe::SideEffectLevel::None,
                        args: serde_json::json!({}),
                    },
                    kind: seaki_pipe::DagNodeKind::Join { strategy },
                    predecessors: preds,
                    successors: succs,
                }
            }
            Node::Exit { .. } => seaki_pipe::DagStep {
                composed: seaki_pipe::ComposedStep {
                    step_id: node_id.0.clone(),
                    command_id: String::new(),
                    input_type: (
                        seaki_pipe::FrameType::JsonValue,
                        seaki_pipe::Cardinality::One,
                    ),
                    output_type: (
                        seaki_pipe::FrameType::JsonValue,
                        seaki_pipe::Cardinality::One,
                    ),
                    input_binding: seaki_pipe::InputBinding::PreviousStep,
                    failure_policy: seaki_pipe::FailurePolicy::FailFast,
                    side_effect_level: seaki_pipe::SideEffectLevel::None,
                    args: serde_json::json!({}),
                },
                kind: seaki_pipe::DagNodeKind::Exit,
                predecessors: preds,
                successors: succs,
            },
            Node::Entry { .. } => unreachable!(),
        };

        steps.push(dag_step);
    }

    if let Some(first) = steps.first() {
        input_type = first.composed.input_type;
    }
    if let Some(last) = steps.last() {
        output_type = last.composed.output_type;
    }

    Ok(seaki_pipe::DagPipeline {
        pipeline_id: graph.graph_id.clone(),
        steps,
        input_type,
        output_type,
    })
}

fn topological_sort(graph: &PipelineGraph) -> Result<Vec<NodeId>, CompileError> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for node_id in graph.node_ids() {
        in_degree.entry(node_id.0.clone()).or_insert(0);
    }

    for edge in &graph.edges {
        adj.entry(edge.from.0.clone())
            .or_default()
            .push(edge.to.0.clone());
        *in_degree.entry(edge.to.0.clone()).or_insert(0) += 1;
    }

    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut result = Vec::new();

    while let Some(id) = queue.pop() {
        result.push(NodeId::from(id.clone()));
        if let Some(neighbors) = adj.get(&id) {
            for neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(neighbor.clone());
                    }
                }
            }
        }
    }

    if result.len() != graph.node_count() {
        return Err(CompileError::Graph(GraphError::CycleDetected));
    }

    Ok(result)
}

/// Validate that a manifest's stored `schema_hash` matches the computed hash.
fn validate_manifest_hash(manifest: &PipeCommandManifest) -> Result<(), CompileError> {
    let expected_hash =
        PipeCommandManifest::compute_schema_hash(&manifest.input_schema, &manifest.output_schema);
    if manifest.schema_hash != expected_hash {
        return Err(CompileError::SchemaHashMismatch {
            command_id: manifest.command_id.clone(),
            expected: expected_hash,
            found: manifest.schema_hash.clone(),
        });
    }
    Ok(())
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

    // Validate schema hashes before delegating to compose, and cache manifests
    // to avoid re-looking them up after compose.
    let mut manifest_cache: HashMap<String, (Option<ResourceQuota>, String)> = HashMap::new();
    for step in &ast.steps {
        let manifest = registry.inspect(&step.command_id).map_err(|_| {
            CompileError::Compose(seaki_pipe::ComposeError::CommandNotFound(
                step.command_id.clone(),
            ))
        })?;
        validate_manifest_hash(manifest)?;
        manifest_cache.insert(
            step.command_id.clone(),
            (
                manifest.resource_quota.clone(),
                manifest.schema_hash.clone(),
            ),
        );
    }

    // Delegate type-checking to seaki_pipe::compose.
    let composed = seaki_pipe::compose(&ast, registry)?;

    let mut linear_steps: Vec<CompiledStep> = Vec::with_capacity(composed.steps.len());
    let mut max_side_effect = SideEffectLevel::None;
    let mut command_schema_hashes: HashMap<String, String> = HashMap::new();

    for step in &composed.steps {
        let (resource_quota, schema_hash) =
            manifest_cache.get(&step.command_id).ok_or_else(|| {
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
            resource_quota: resource_quota.clone(),
            schema_hash: schema_hash.clone(),
        });

        command_schema_hashes.insert(step.command_id.clone(), schema_hash.clone());

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
