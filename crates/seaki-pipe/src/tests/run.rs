use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ast::{
    compose, Cardinality, ComposedPipeline, ComposedStep, DagMergeStrategy, DagNodeKind,
    DagPipeline, DagStep, FailurePolicy, FrameType, InputBinding, PipelineAst, PipelineStep,
    TypedFrame,
};
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::registry::{CommandRegistry, PipeCommandManifest, SideEffectLevel};
use crate::run::*;
use crate::ErrorKind;

fn test_context() -> ExecutionContext {
    ExecutionContext {
        workspace_id: "ws-1".to_string(),
        actor_id: "actor-1".to_string(),
        pipeline_id: "pipe-1".to_string(),
        audit: Vec::new(),
        resource_used: ResourceUsage::default(),
    }
}

fn builtin_executors() -> HashMap<String, Box<dyn CommandExecutor>> {
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

fn registry_with_failing() -> CommandRegistry {
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
struct FailingExecutor;

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

#[test]
fn run_empty_pipeline_fails() {
    let registry = CommandRegistry::builtin();
    let pipeline = ComposedPipeline {
        pipeline_id: "empty".to_string(),
        steps: vec![],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::JsonValue, Cardinality::One),
    };
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run(
        &pipeline,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    );
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err().error_kind,
        ErrorKind::ComposeFailed
    ));
}

#[test]
fn run_single_step_success() {
    let registry = CommandRegistry::builtin();
    let ast = PipelineAst {
        pipeline_id: "test".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "filter".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!([{"x": 1}, {"x": 2}])),
            failure_policy: FailurePolicy::FailFast,
            args: serde_json::json!({"predicate": {"x": 1}}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    assert_eq!(result.output.len(), 1);
    assert_eq!(result.output[0].payload, serde_json::json!({"x": 1}));
    assert_eq!(ctx.audit.len(), 1);
    assert_eq!(ctx.audit[0].step_id, "s1");
}

#[test]
fn run_multi_step_chain_success() {
    let registry = CommandRegistry::builtin();
    let ast = PipelineAst {
        pipeline_id: "chain".to_string(),
        steps: vec![
            PipelineStep {
                step_id: "s1".to_string(),
                command_id: "map".to_string(),
                input_binding: InputBinding::Constant(serde_json::json!([{"x": 1}, {"x": 2}])),
                failure_policy: FailurePolicy::FailFast,
                args: serde_json::json!({"transform": {"tag": "a"}}),
            },
            PipelineStep {
                step_id: "s2".to_string(),
                command_id: "filter".to_string(),
                input_binding: InputBinding::PreviousStep,
                failure_policy: FailurePolicy::FailFast,
                args: serde_json::json!({"predicate": {"tag": "a"}}),
            },
        ],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    assert_eq!(result.output.len(), 2);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "tag": "a"})
    );
    assert_eq!(
        result.output[1].payload,
        serde_json::json!({"x": 2, "tag": "a"})
    );
    assert_eq!(ctx.audit.len(), 2);
}

#[test]
fn run_fail_fast_propagates_error() {
    let registry = registry_with_failing();
    let ast = PipelineAst {
        pipeline_id: "fail".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "failing".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!({})),
            failure_policy: FailurePolicy::FailFast,
            args: serde_json::json!({}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let mut executors = HashMap::new();
    executors.insert(
        "failing".to_string(),
        Box::new(FailingExecutor) as Box<dyn CommandExecutor>,
    );
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.failed_step_id, "s1");
    assert!(matches!(err.error_kind, ErrorKind::CommandNotFound));
}

#[test]
fn run_skip_policy_continues() {
    let mut registry = registry_with_failing();
    // Add a map command so the second step can run.
    let map_input = serde_json::json!({"type": "array"});
    let map_output = serde_json::json!({"type": "array"});
    let map_manifest = PipeCommandManifest {
        command_id: "map".to_string(),
        description: "map".to_string(),
        input_schema: map_input.clone(),
        output_schema: map_output.clone(),
        input_frame: (FrameType::JsonValue, Cardinality::Many),
        output_frame: (FrameType::JsonValue, Cardinality::Many),
        side_effect_level: SideEffectLevel::None,
        resource_quota: None,
        schema_hash: PipeCommandManifest::compute_schema_hash(&map_input, &map_output),
    };
    registry.register(map_manifest).unwrap();

    let ast = PipelineAst {
        pipeline_id: "skip".to_string(),
        steps: vec![
            PipelineStep {
                step_id: "s1".to_string(),
                command_id: "failing".to_string(),
                input_binding: InputBinding::Constant(serde_json::json!([{"x": 1}])),
                failure_policy: FailurePolicy::Skip,
                args: serde_json::json!({}),
            },
            PipelineStep {
                step_id: "s2".to_string(),
                command_id: "map".to_string(),
                input_binding: InputBinding::PreviousStep,
                failure_policy: FailurePolicy::FailFast,
                args: serde_json::json!({"transform": {"recovered": true}}),
            },
        ],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let mut executors = HashMap::new();
    executors.insert(
        "failing".to_string(),
        Box::new(FailingExecutor) as Box<dyn CommandExecutor>,
    );
    executors.insert(
        "map".to_string(),
        Box::new(MapExecutor) as Box<dyn CommandExecutor>,
    );
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    // s1 skipped -> empty output; s2 map on empty -> empty output.
    assert!(result.output.is_empty());
    // Audit should contain both the skip record for s1 and the allow record for s2.
    assert!(ctx
        .audit
        .iter()
        .any(|a| a.step_id == "s1" && a.decision.starts_with("skipped")));
    assert!(ctx
        .audit
        .iter()
        .any(|a| a.step_id == "s2" && a.decision == "allow"));
}

#[test]
fn run_default_value_on_failure() {
    let registry = registry_with_failing();
    let ast = PipelineAst {
        pipeline_id: "default".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "failing".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!({})),
            failure_policy: FailurePolicy::Default(serde_json::json!({"default": true})),
            args: serde_json::json!({}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let mut executors = HashMap::new();
    executors.insert(
        "failing".to_string(),
        Box::new(FailingExecutor) as Box<dyn CommandExecutor>,
    );
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    assert_eq!(result.output.len(), 1);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"default": true})
    );
}

#[test]
fn run_require_approval_blocks_execution() {
    let registry = CommandRegistry::builtin();
    // wiki.patch.propose has side_effect_level = ProposalOnly, so SimplePolicy
    // returns RequireApproval.
    let ast = PipelineAst {
        pipeline_id: "approval".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "wiki.patch.propose".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!([])),
            failure_policy: FailurePolicy::FailFast,
            args: serde_json::json!({}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.failed_step_id, "s1");
    assert!(matches!(err.error_kind, ErrorKind::ApprovalRequired));
    assert!(err.retryable);
    // Executor must NOT have been called — no audit for successful execution.
    assert!(!ctx
        .audit
        .iter()
        .any(|a| a.step_id == "s1" && a.decision == "allow"));
}

#[test]
fn run_resource_exceeded_terminates() {
    let registry = CommandRegistry::builtin();
    // Create an array with 1_001 elements to exceed MAX_FRAME_COUNT.
    let large_array: Vec<serde_json::Value> =
        (0..1_001).map(|i| serde_json::json!({"idx": i})).collect();

    let ast = PipelineAst {
        pipeline_id: "res".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "filter".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!(large_array)),
            failure_policy: FailurePolicy::FailFast,
            args: serde_json::json!({"predicate": {}}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.failed_step_id, "s1");
    assert!(matches!(
        err.error_kind,
        ErrorKind::ResourceExceeded { ref limit, .. } if limit == "frame_count"
    ));
}

#[test]
fn run_filter_executor_real() {
    let registry = CommandRegistry::builtin();
    let ast = PipelineAst {
        pipeline_id: "filter".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "filter".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!([
                {"name": "alice", "age": 30},
                {"name": "bob", "age": 25},
                {"name": "alice", "age": 40}
            ])),
            failure_policy: FailurePolicy::FailFast,
            args: serde_json::json!({"predicate": {"name": "alice"}}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    assert_eq!(result.output.len(), 2);
    assert!(result
        .output
        .iter()
        .all(|f| f.payload.get("name") == Some(&serde_json::json!("alice"))));
}

#[test]
fn run_map_executor_real() {
    let registry = CommandRegistry::builtin();
    let ast = PipelineAst {
        pipeline_id: "map".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "map".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!([
                {"x": 1},
                {"x": 2}
            ])),
            failure_policy: FailurePolicy::FailFast,
            args: serde_json::json!({"transform": {"y": 10}}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run(
        &composed,
        serde_json::json!({}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    assert_eq!(result.output.len(), 2);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "y": 10})
    );
    assert_eq!(
        result.output[1].payload,
        serde_json::json!({"x": 2, "y": 10})
    );
}

// ============================================================================
// DAG runtime tests
// ============================================================================

#[allow(clippy::too_many_arguments)]
fn make_dag_step(
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

#[test]
fn dag_tee_executes_both_branches() {
    let dag = DagPipeline {
        pipeline_id: "tee_test".to_string(),
        steps: vec![
            make_dag_step(
                "A",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"src": "A"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["tee"],
            ),
            make_dag_step(
                "tee",
                DagNodeKind::Tee,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["A"],
                vec!["B", "C"],
            ),
            make_dag_step(
                "B",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"src": "B"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["join"],
            ),
            make_dag_step(
                "C",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"src": "C"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["join"],
            ),
            make_dag_step(
                "join",
                DagNodeKind::Join {
                    strategy: DagMergeStrategy::Concat,
                },
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["B", "C"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["join"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run_dag(
        &dag,
        serde_json::json!([{"x": 1}, {"x": 2}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    // A -> [{x:1, src:A}, {x:2, src:A}]
    // tee copies to B and C
    // B -> [{x:1, src:B}, {x:2, src:B}]
    // C -> [{x:1, src:C}, {x:2, src:C}]
    // join concat -> 4 frames
    assert_eq!(result.output.len(), 4);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "src": "B"})
    );
    assert_eq!(
        result.output[1].payload,
        serde_json::json!({"x": 2, "src": "B"})
    );
    assert_eq!(
        result.output[2].payload,
        serde_json::json!({"x": 1, "src": "C"})
    );
    assert_eq!(
        result.output[3].payload,
        serde_json::json!({"x": 2, "src": "C"})
    );
}

#[test]
fn dag_branch_conditional() {
    let dag = DagPipeline {
        pipeline_id: "branch_test".to_string(),
        steps: vec![
            make_dag_step(
                "A",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"tag": "A"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["branch"],
            ),
            make_dag_step(
                "branch",
                DagNodeKind::Branch,
                "",
                serde_json::json!({
                    "branches": [
                        {"target": "B", "name": "left"},
                        {"target": "C", "name": "right"}
                    ],
                    "route": "left"
                }),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["A"],
                vec!["B", "C"],
            ),
            make_dag_step(
                "B",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"branch": "B"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["branch"],
                vec!["exit"],
            ),
            make_dag_step(
                "C",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"branch": "C"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["branch"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["B", "C"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run_dag(
        &dag,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    // branch selects B, C is skipped
    assert_eq!(result.output.len(), 1);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "tag": "A", "branch": "B"})
    );
}

#[test]
fn dag_join_concat() {
    let dag = DagPipeline {
        pipeline_id: "join_concat".to_string(),
        steps: vec![
            make_dag_step(
                "A",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"src": "A"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["tee"],
            ),
            make_dag_step(
                "tee",
                DagNodeKind::Tee,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["A"],
                vec!["B", "C"],
            ),
            make_dag_step(
                "B",
                DagNodeKind::Command,
                "filter",
                serde_json::json!({"predicate": {"x": 1}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["join"],
            ),
            make_dag_step(
                "C",
                DagNodeKind::Command,
                "filter",
                serde_json::json!({"predicate": {"x": 2}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["join"],
            ),
            make_dag_step(
                "join",
                DagNodeKind::Join {
                    strategy: DagMergeStrategy::Concat,
                },
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["B", "C"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["join"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run_dag(
        &dag,
        serde_json::json!([{"x": 1}, {"x": 2}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    // A adds src:A to both frames
    // B filters x:1 -> [{x:1, src:A}]
    // C filters x:2 -> [{x:2, src:A}]
    // join concat -> B then C
    assert_eq!(result.output.len(), 2);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "src": "A"})
    );
    assert_eq!(
        result.output[1].payload,
        serde_json::json!({"x": 2, "src": "A"})
    );
}

#[test]
fn dag_join_first_non_empty() {
    let dag = DagPipeline {
        pipeline_id: "join_first".to_string(),
        steps: vec![
            make_dag_step(
                "A",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"src": "A"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["tee"],
            ),
            make_dag_step(
                "tee",
                DagNodeKind::Tee,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["A"],
                vec!["B", "C"],
            ),
            make_dag_step(
                "B",
                DagNodeKind::Command,
                "filter",
                serde_json::json!({"predicate": {"x": 1}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["join"],
            ),
            make_dag_step(
                "C",
                DagNodeKind::Command,
                "filter",
                serde_json::json!({"predicate": {"x": 99}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["join"],
            ),
            make_dag_step(
                "join",
                DagNodeKind::Join {
                    strategy: DagMergeStrategy::FirstNonEmpty,
                },
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["B", "C"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["join"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run_dag(
        &dag,
        serde_json::json!([{"x": 1}, {"x": 2}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    // B -> [{x:1, src:A}] (non-empty)
    // C -> [] (empty, predicate x:99 matches nothing)
    // FirstNonEmpty -> B's output
    assert_eq!(result.output.len(), 1);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "src": "A"})
    );
}

#[test]
fn dag_complex_mixed() {
    let dag = DagPipeline {
        pipeline_id: "complex".to_string(),
        steps: vec![
            make_dag_step(
                "A",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"stage": "A"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["tee"],
            ),
            make_dag_step(
                "tee",
                DagNodeKind::Tee,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["A"],
                vec!["B", "branch"],
            ),
            make_dag_step(
                "B",
                DagNodeKind::Command,
                "filter",
                serde_json::json!({"predicate": {"stage": "A"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["join"],
            ),
            make_dag_step(
                "branch",
                DagNodeKind::Branch,
                "",
                serde_json::json!({
                    "branches": [
                        {"target": "C", "name": "c_path"},
                        {"target": "D", "name": "d_path"}
                    ],
                    "route": "c_path"
                }),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["tee"],
                vec!["C", "D"],
            ),
            make_dag_step(
                "C",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"stage": "C"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["branch"],
                vec!["join"],
            ),
            make_dag_step(
                "D",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"stage": "D"}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["branch"],
                vec!["join"],
            ),
            make_dag_step(
                "join",
                DagNodeKind::Join {
                    strategy: DagMergeStrategy::Concat,
                },
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["B", "C"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["join"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let result = run_dag(
        &dag,
        serde_json::json!([{"x": 1}, {"x": 2}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    // A -> [{x:1, stage:A}, {x:2, stage:A}]
    // tee -> B and branch
    // B filters stage:A -> both frames pass -> [{x:1, stage:A}, {x:2, stage:A}]
    // branch selects C, D is skipped
    // C -> [{x:1, stage:C}, {x:2, stage:C}]
    // join concat B + C -> 4 frames
    assert_eq!(result.output.len(), 4);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "stage": "A"})
    );
    assert_eq!(
        result.output[1].payload,
        serde_json::json!({"x": 2, "stage": "A"})
    );
    assert_eq!(
        result.output[2].payload,
        serde_json::json!({"x": 1, "stage": "C"})
    );
    assert_eq!(
        result.output[3].payload,
        serde_json::json!({"x": 2, "stage": "C"})
    );
}

// ============================================================================
// Checkpoint / Resume / Retry / Compensation tests
// ============================================================================

#[test]
fn checkpoint_saves_after_each_step() {
    let dag = DagPipeline {
        pipeline_id: "chk".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"a": 1}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["s2"],
            ),
            make_dag_step(
                "s2",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"b": 2}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s1"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s2"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };
    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let store = InMemoryCheckpointStore::default();

    let result = run_dag_with_checkpoint(
        &dag,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
        &store,
        None,
    )
    .unwrap();

    assert_eq!(result.output.len(), 1);

    let checkpoints = store.load_all("chk").unwrap();
    assert_eq!(checkpoints.len(), 3);
    assert_eq!(checkpoints[0].step_id, "s1");
    assert_eq!(checkpoints[1].step_id, "s2");
    assert_eq!(checkpoints[2].step_id, "exit");
}

#[test]
fn resume_skips_completed_steps() {
    let dag = DagPipeline {
        pipeline_id: "res".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"a": 1}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["s2"],
            ),
            make_dag_step(
                "s2",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"b": 2}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s1"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s2"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };
    let store = InMemoryCheckpointStore::default();
    let cp1 = Checkpoint {
        pipeline_id: "res".to_string(),
        step_id: "s1".to_string(),
        output_frames: vec![FrameEnvelope {
            seq: 0,
            step_id: "s1".to_string(),
            frame_type: FrameType::JsonValue,
            payload: serde_json::json!({"x": 1, "a": 1}),
        }],
        step_state: StepState::Completed,
        resource_used: ResourceUsage {
            cpu_ms: 10,
            memory_mb: 0,
            frame_count: 1,
        },
        timestamp_ms: 1,
    };
    store.save(&cp1).unwrap();

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();

    let result = resume_dag(
        &dag,
        &store,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    // s1 skipped, s2 and exit executed
    assert_eq!(result.output.len(), 1);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"x": 1, "a": 1, "b": 2})
    );
}

#[test]
fn resume_updates_resource_usage() {
    let dag = DagPipeline {
        pipeline_id: "res".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"a": 1}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s1"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };
    let store = InMemoryCheckpointStore::default();
    let cp1 = Checkpoint {
        pipeline_id: "res".to_string(),
        step_id: "s1".to_string(),
        output_frames: vec![],
        step_state: StepState::Completed,
        resource_used: ResourceUsage {
            cpu_ms: 42,
            memory_mb: 5,
            frame_count: 3,
        },
        timestamp_ms: 1,
    };
    store.save(&cp1).unwrap();

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();

    resume_dag(
        &dag,
        &store,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
    )
    .unwrap();

    assert_eq!(ctx.resource_used.cpu_ms, 42);
    assert_eq!(ctx.resource_used.memory_mb, 5);
    assert_eq!(ctx.resource_used.frame_count, 3);
}

#[derive(Debug)]
struct FlakyExecutor {
    fail_count: AtomicUsize,
}

impl CommandExecutor for FlakyExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        let remaining = self.fail_count.fetch_sub(1, Ordering::SeqCst);
        if remaining > 0 {
            Err(PipelineError {
                retryable: true,
                failed_step_id: step.step_id.clone(),
                error_kind: ErrorKind::QuotaExceeded,
            })
        } else {
            Ok(vec![FrameEnvelope {
                seq: 0,
                step_id: step.step_id.clone(),
                frame_type: FrameType::JsonValue,
                payload: serde_json::json!({"recovered": true}),
            }])
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AlwaysRetryableExecutor;

impl CommandExecutor for AlwaysRetryableExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Err(PipelineError {
            retryable: true,
            failed_step_id: step.step_id.clone(),
            error_kind: ErrorKind::QuotaExceeded,
        })
    }
}

#[test]
fn retry_recoverable_error() {
    let dag = DagPipeline {
        pipeline_id: "retry".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "flaky",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::One),
                (FrameType::JsonValue, Cardinality::One),
                vec!["entry"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::One),
                (FrameType::JsonValue, Cardinality::One),
                vec!["s1"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::JsonValue, Cardinality::One),
    };

    let mut registry = CommandRegistry::new();
    let input = serde_json::json!({"type": "object"});
    let output = serde_json::json!({"type": "object"});
    let manifest = PipeCommandManifest {
        command_id: "flaky".to_string(),
        description: "flaky".to_string(),
        input_schema: input.clone(),
        output_schema: output.clone(),
        input_frame: (FrameType::JsonValue, Cardinality::One),
        output_frame: (FrameType::JsonValue, Cardinality::One),
        side_effect_level: SideEffectLevel::None,
        resource_quota: None,
        schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
    };
    registry.register(manifest).unwrap();

    let mut ctx = test_context();
    let mut executors: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    executors.insert(
        "flaky".to_string(),
        Box::new(FlakyExecutor {
            fail_count: AtomicUsize::new(2),
        }),
    );

    let retry_policy = RetryPolicy {
        max_attempts: 3,
        backoff_ms: 0,
    };
    let result = run_dag_with_checkpoint(
        &dag,
        serde_json::json!({"x": 1}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
        &InMemoryCheckpointStore::default(),
        Some(&retry_policy),
    )
    .unwrap();

    assert_eq!(result.output.len(), 1);
    assert_eq!(
        result.output[0].payload,
        serde_json::json!({"recovered": true})
    );
}

#[test]
fn retry_exhausted_returns_last_error() {
    let dag = DagPipeline {
        pipeline_id: "retry".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "always_retryable",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::One),
                (FrameType::JsonValue, Cardinality::One),
                vec!["entry"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::One),
                (FrameType::JsonValue, Cardinality::One),
                vec!["s1"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::JsonValue, Cardinality::One),
    };

    let mut registry = CommandRegistry::new();
    let input = serde_json::json!({"type": "object"});
    let output = serde_json::json!({"type": "object"});
    let manifest = PipeCommandManifest {
        command_id: "always_retryable".to_string(),
        description: "always fails retryable".to_string(),
        input_schema: input.clone(),
        output_schema: output.clone(),
        input_frame: (FrameType::JsonValue, Cardinality::One),
        output_frame: (FrameType::JsonValue, Cardinality::One),
        side_effect_level: SideEffectLevel::None,
        resource_quota: None,
        schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
    };
    registry.register(manifest).unwrap();

    let mut ctx = test_context();
    let mut executors: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    executors.insert(
        "always_retryable".to_string(),
        Box::new(AlwaysRetryableExecutor),
    );

    let retry_policy = RetryPolicy {
        max_attempts: 3,
        backoff_ms: 0,
    };
    let result = run_dag_with_checkpoint(
        &dag,
        serde_json::json!({"x": 1}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
        &InMemoryCheckpointStore::default(),
        Some(&retry_policy),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.retryable);
    assert_eq!(err.failed_step_id, "s1");
    assert!(matches!(err.error_kind, ErrorKind::QuotaExceeded));
}

#[test]
fn retry_non_retryable_no_retry() {
    let dag = DagPipeline {
        pipeline_id: "retry".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "failing",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::One),
                (FrameType::JsonValue, Cardinality::One),
                vec!["entry"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::One),
                (FrameType::JsonValue, Cardinality::One),
                vec!["s1"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::JsonValue, Cardinality::One),
    };

    let registry = registry_with_failing();
    let mut ctx = test_context();
    let mut executors = HashMap::new();
    executors.insert(
        "failing".to_string(),
        Box::new(FailingExecutor) as Box<dyn CommandExecutor>,
    );

    let retry_policy = RetryPolicy {
        max_attempts: 3,
        backoff_ms: 0,
    };
    let result = run_dag_with_checkpoint(
        &dag,
        serde_json::json!({"x": 1}),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
        &InMemoryCheckpointStore::default(),
        Some(&retry_policy),
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(!err.retryable);
    assert_eq!(err.failed_step_id, "s1");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompensatingMapExecutor;

impl CommandExecutor for CompensatingMapExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        MapExecutor.execute(step, input, ctx)
    }
}

impl CompensatingExecutor for CompensatingMapExecutor {
    fn compensate(
        &self,
        step: &ComposedStep,
        _executed_output: &[FrameEnvelope],
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![FrameEnvelope {
            seq: 0,
            step_id: step.step_id.clone(),
            frame_type: FrameType::JsonValue,
            payload: serde_json::json!({"compensated": step.step_id}),
        }])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FailingCompensateExecutor;

impl CommandExecutor for FailingCompensateExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        MapExecutor.execute(step, input, ctx)
    }
}

impl CompensatingExecutor for FailingCompensateExecutor {
    fn compensate(
        &self,
        step: &ComposedStep,
        _executed_output: &[FrameEnvelope],
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Err(PipelineError {
            retryable: false,
            failed_step_id: step.step_id.clone(),
            error_kind: ErrorKind::SideEffectBlocked,
        })
    }
}

#[test]
fn rollback_calls_compensate_in_reverse_order() {
    let dag = DagPipeline {
        pipeline_id: "rollback".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"a": 1}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["s2"],
            ),
            make_dag_step(
                "s2",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"b": 2}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s1"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s2"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };

    let registry = CommandRegistry::builtin();
    let mut ctx = test_context();
    let executors = builtin_executors();
    let store = InMemoryCheckpointStore::default();

    run_dag_with_checkpoint(
        &dag,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
        &store,
        None,
    )
    .unwrap();

    let mut ctx2 = test_context();
    let mut compensators: HashMap<String, Box<dyn CompensatingExecutor>> = HashMap::new();
    compensators.insert("map".to_string(), Box::new(CompensatingMapExecutor));

    let records = rollback_dag(&dag, &store, &registry, &compensators, &mut ctx2).unwrap();

    // Reverse topological: s2, s1 (exit has no compensator)
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].step_id, "s2");
    assert!(records[0].success);
    assert_eq!(records[1].step_id, "s1");
    assert!(records[1].success);
}

#[test]
fn rollback_continues_on_compensation_failure() {
    let dag = DagPipeline {
        pipeline_id: "rollback".to_string(),
        steps: vec![
            make_dag_step(
                "s1",
                DagNodeKind::Command,
                "map",
                serde_json::json!({"transform": {"a": 1}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["entry"],
                vec!["s2"],
            ),
            make_dag_step(
                "s2",
                DagNodeKind::Command,
                "failing_comp",
                serde_json::json!({"transform": {"b": 2}}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s1"],
                vec!["exit"],
            ),
            make_dag_step(
                "exit",
                DagNodeKind::Exit,
                "",
                serde_json::json!({}),
                (FrameType::JsonValue, Cardinality::Many),
                (FrameType::JsonValue, Cardinality::Many),
                vec!["s2"],
                vec![],
            ),
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    };

    let mut registry = CommandRegistry::builtin();
    let input = serde_json::json!({"type": "array"});
    let output = serde_json::json!({"type": "array"});
    let manifest = PipeCommandManifest {
        command_id: "failing_comp".to_string(),
        description: "failing compensate".to_string(),
        input_schema: input.clone(),
        output_schema: output.clone(),
        input_frame: (FrameType::JsonValue, Cardinality::Many),
        output_frame: (FrameType::JsonValue, Cardinality::Many),
        side_effect_level: SideEffectLevel::None,
        resource_quota: None,
        schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
    };
    registry.register(manifest).unwrap();

    let mut ctx = test_context();
    let mut executors: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    executors.insert("map".to_string(), Box::new(MapExecutor));
    executors.insert(
        "failing_comp".to_string(),
        Box::new(FailingCompensateExecutor),
    );

    let store = InMemoryCheckpointStore::default();

    run_dag_with_checkpoint(
        &dag,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &mut ctx,
        &store,
        None,
    )
    .unwrap();

    let mut ctx2 = test_context();
    let mut compensators: HashMap<String, Box<dyn CompensatingExecutor>> = HashMap::new();
    compensators.insert("map".to_string(), Box::new(CompensatingMapExecutor));
    compensators.insert(
        "failing_comp".to_string(),
        Box::new(FailingCompensateExecutor),
    );

    let records = rollback_dag(&dag, &store, &registry, &compensators, &mut ctx2).unwrap();

    // Reverse order: s2 (fails), s1 (succeeds)
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].step_id, "s2");
    assert!(!records[0].success);
    assert_eq!(records[1].step_id, "s1");
    assert!(records[1].success);
}
