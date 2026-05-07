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
