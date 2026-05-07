use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

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
