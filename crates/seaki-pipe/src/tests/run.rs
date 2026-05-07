use super::*;

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
fn run_frame_size_exceeded() {
    let registry = CommandRegistry::builtin();
    let large_string = "x".repeat(2 * 1_024 * 1_024);
    let ast = PipelineAst {
        pipeline_id: "frame_size".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "filter".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!([{"data": large_string}])),
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
    assert!(matches!(
        err.error_kind,
        ErrorKind::ResourceExceeded { ref limit, .. } if limit == "frame_size"
    ));
}

#[test]
fn run_cpu_ms_exceeded() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SlowExecutor;

    impl CommandExecutor for SlowExecutor {
        fn execute(
            &self,
            _step: &ComposedStep,
            input: Vec<FrameEnvelope>,
            _ctx: &mut ExecutionContext,
        ) -> Result<Vec<FrameEnvelope>, PipelineError> {
            std::thread::sleep(std::time::Duration::from_millis(50));
            Ok(input)
        }
    }

    let mut registry = CommandRegistry::new();
    let input = serde_json::json!({"type": "object"});
    let output = serde_json::json!({"type": "object"});
    let manifest = PipeCommandManifest {
        command_id: "slow".to_string(),
        description: "slow".to_string(),
        input_schema: input.clone(),
        output_schema: output.clone(),
        input_frame: (FrameType::JsonValue, Cardinality::One),
        output_frame: (FrameType::JsonValue, Cardinality::One),
        side_effect_level: SideEffectLevel::None,
        resource_quota: Some(ResourceQuota {
            cpu_ms: 10,
            memory_mb: 1_024,
        }),
        schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
    };
    registry.register(manifest).unwrap();

    let ast = PipelineAst {
        pipeline_id: "cpu".to_string(),
        steps: vec![PipelineStep {
            step_id: "s1".to_string(),
            command_id: "slow".to_string(),
            input_binding: InputBinding::Constant(serde_json::json!({})),
            failure_policy: FailurePolicy::FailFast,
            args: serde_json::json!({}),
        }],
    };
    let composed = compose(&ast, &registry).unwrap();
    let mut ctx = test_context();
    let mut executors = HashMap::new();
    executors.insert(
        "slow".to_string(),
        Box::new(SlowExecutor) as Box<dyn CommandExecutor>,
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
    assert!(matches!(
        err.error_kind,
        ErrorKind::ResourceExceeded { ref limit, .. } if limit == "cpu_ms"
    ));
}
