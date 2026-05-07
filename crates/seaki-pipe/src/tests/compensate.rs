use std::collections::HashMap;

use super::*;

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
