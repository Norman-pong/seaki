use super::*;

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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::approval_gate::{ApprovalGate, ApprovalGateError, ApprovalRequestInput};
use crate::checkpoint::InMemoryCheckpointStore;
use crate::registry::{PipeCommandManifest, ResourceQuota, SideEffectLevel};
use crate::run::CompensatingExecutor;
use crate::state_machine::{PipelineState, PipelineStateMachine};
use seaki_policy::ApprovalStatus;

/// Approval gate that auto-approves after a short delay.
struct DelayedApproveGate {
    delay_ms: u64,
}

impl ApprovalGate for DelayedApproveGate {
    fn request_approval(
        &self,
        _request: ApprovalRequestInput,
    ) -> Result<String, ApprovalGateError> {
        Ok("delayed-approval".to_string())
    }

    fn poll_approval(&self, _approval_id: &str) -> Result<ApprovalStatus, ApprovalGateError> {
        Ok(ApprovalStatus::Pending)
    }

    fn wait_for_approval(
        &self,
        _approval_id: &str,
        _timeout_ms: u64,
    ) -> Result<ApprovalStatus, ApprovalGateError> {
        std::thread::sleep(std::time::Duration::from_millis(self.delay_ms));
        Ok(ApprovalStatus::Approved)
    }
}

/// Approval gate that auto-denies immediately.
struct AutoDenyGate;

impl ApprovalGate for AutoDenyGate {
    fn request_approval(
        &self,
        _request: ApprovalRequestInput,
    ) -> Result<String, ApprovalGateError> {
        Ok("auto-deny".to_string())
    }

    fn poll_approval(&self, _approval_id: &str) -> Result<ApprovalStatus, ApprovalGateError> {
        Ok(ApprovalStatus::Denied)
    }

    fn wait_for_approval(
        &self,
        _approval_id: &str,
        _timeout_ms: u64,
    ) -> Result<ApprovalStatus, ApprovalGateError> {
        Ok(ApprovalStatus::Denied)
    }
}

/// Compensator that tracks which steps were compensated.
#[derive(Debug, Clone)]
struct TrackingCompensator {
    compensated: Arc<Mutex<Vec<String>>>,
}

impl TrackingCompensator {
    fn new() -> Self {
        Self {
            compensated: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn compensated_steps(&self) -> Vec<String> {
        self.compensated.lock().unwrap().clone()
    }
}

impl CommandExecutor for TrackingCompensator {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        MapExecutor.execute(step, input, ctx)
    }
}

impl CompensatingExecutor for TrackingCompensator {
    fn compensate(
        &self,
        step: &ComposedStep,
        _executed_output: &[FrameEnvelope],
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        self.compensated.lock().unwrap().push(step.step_id.clone());
        Ok(Vec::new())
    }
}

fn approval_dag() -> DagPipeline {
    DagPipeline {
        pipeline_id: "approval_test".to_string(),
        steps: vec![
            DagStep {
                composed: ComposedStep {
                    step_id: "s1".to_string(),
                    command_id: "map".to_string(),
                    input_type: (FrameType::JsonValue, Cardinality::Many),
                    output_type: (FrameType::JsonValue, Cardinality::Many),
                    input_binding: InputBinding::PreviousStep,
                    failure_policy: FailurePolicy::FailFast,
                    side_effect_level: SideEffectLevel::None,
                    args: serde_json::json!({"transform": {"stage": "s1"}}),
                },
                kind: DagNodeKind::Command,
                predecessors: vec!["entry".to_string()],
                successors: vec!["s2".to_string()],
            },
            DagStep {
                composed: ComposedStep {
                    step_id: "s2".to_string(),
                    command_id: "needs_approval".to_string(),
                    input_type: (FrameType::JsonValue, Cardinality::Many),
                    output_type: (FrameType::JsonValue, Cardinality::Many),
                    input_binding: InputBinding::PreviousStep,
                    failure_policy: FailurePolicy::FailFast,
                    side_effect_level: SideEffectLevel::SideEffect,
                    args: serde_json::json!({"transform": {"stage": "s2"}}),
                },
                kind: DagNodeKind::Command,
                predecessors: vec!["s1".to_string()],
                successors: vec!["exit".to_string()],
            },
            DagStep {
                composed: ComposedStep {
                    step_id: "exit".to_string(),
                    command_id: "".to_string(),
                    input_type: (FrameType::JsonValue, Cardinality::Many),
                    output_type: (FrameType::JsonValue, Cardinality::Many),
                    input_binding: InputBinding::PreviousStep,
                    failure_policy: FailurePolicy::FailFast,
                    side_effect_level: SideEffectLevel::None,
                    args: serde_json::json!({}),
                },
                kind: DagNodeKind::Exit,
                predecessors: vec!["s2".to_string()],
                successors: vec![],
            },
        ],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
    }
}

fn approval_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::builtin();
    let input = serde_json::json!({"type": "array"});
    let output = serde_json::json!({"type": "array"});
    let manifest = PipeCommandManifest {
        command_id: "needs_approval".to_string(),
        description: "needs approval".to_string(),
        input_schema: input.clone(),
        output_schema: output.clone(),
        input_frame: (FrameType::JsonValue, Cardinality::Many),
        output_frame: (FrameType::JsonValue, Cardinality::Many),
        side_effect_level: SideEffectLevel::SideEffect,
        resource_quota: Some(ResourceQuota {
            cpu_ms: 10_000,
            memory_mb: 1_024,
        }),
        schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
    };
    registry.register(manifest).unwrap();
    registry
}

#[test]
fn dag_approval_gate_blocks_and_resumes() {
    let dag = approval_dag();
    let registry = approval_registry();
    let mut ctx = test_context();
    let mut executors: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    executors.insert("map".to_string(), Box::new(MapExecutor));
    executors.insert("needs_approval".to_string(), Box::new(MapExecutor));

    let gate = DelayedApproveGate { delay_ms: 100 };
    let store = InMemoryCheckpointStore::default();
    let compensators: HashMap<String, Box<dyn CompensatingExecutor>> = HashMap::new();
    let mut sm = PipelineStateMachine::new("approval_test".to_string());

    let result = run_dag_with_approval(
        &dag,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &gate,
        &store,
        &compensators,
        &mut ctx,
        &mut sm,
    );

    assert!(result.is_ok(), "expected Ok, got {:?}", result);
    let run_result = result.unwrap();
    assert_eq!(run_result.output.len(), 1);
    assert_eq!(
        run_result.output[0].payload,
        serde_json::json!({"x": 1, "stage": "s2"})
    );
    assert_eq!(sm.state, PipelineState::Completed);
}

#[test]
fn dag_approval_denied_triggers_compensate() {
    let dag = approval_dag();
    let registry = approval_registry();
    let mut ctx = test_context();
    let mut executors: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    executors.insert("map".to_string(), Box::new(MapExecutor));
    executors.insert("needs_approval".to_string(), Box::new(MapExecutor));

    let gate = AutoDenyGate;
    let store = InMemoryCheckpointStore::default();
    let tracker = TrackingCompensator::new();
    let mut compensators: HashMap<String, Box<dyn CompensatingExecutor>> = HashMap::new();
    compensators.insert("map".to_string(), Box::new(tracker.clone()));
    let mut sm = PipelineStateMachine::new("approval_test".to_string());

    let result = run_dag_with_approval(
        &dag,
        serde_json::json!([{"x": 1}]),
        &registry,
        &executors,
        &SimplePolicy,
        &gate,
        &store,
        &compensators,
        &mut ctx,
        &mut sm,
    );

    assert!(result.is_err());
    assert_eq!(sm.state, PipelineState::Failed);

    // Verify that rollback_dag was triggered by checking compensator calls.
    // run_dag_with_approval calls rollback_dag internally when approval is denied.
    let compensated = tracker.compensated_steps();
    assert!(
        compensated.contains(&"s1".to_string()),
        "expected s1 to be compensated, got {:?}",
        compensated
    );
}
