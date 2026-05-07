use super::*;

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
