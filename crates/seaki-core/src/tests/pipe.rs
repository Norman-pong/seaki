use super::*;

#[test]
fn pipe_list_enumerates_builtin_commands() {
    let ledger = initialized_ledger();
    let results = ledger.pipe_list(None);
    assert_eq!(results.len(), 6);
    let ids: Vec<_> = results.iter().map(|r| r.command_id.as_str()).collect();
    assert!(ids.contains(&"wiki.search"));
    assert!(ids.contains(&"wiki.patch.propose"));
}

#[test]
fn pipe_list_filters_by_side_effect_level() {
    let ledger = initialized_ledger();
    let results = ledger.pipe_list(Some(&seaki_pipe::SideEffectFilter::Level(
        seaki_pipe::SideEffectLevel::None,
    )));
    assert_eq!(results.len(), 5);
    for r in &results {
        assert_eq!(r.side_effect_level, "none");
    }
}

#[test]
fn pipe_inspect_returns_full_manifest() {
    let ledger = initialized_ledger();
    let manifest = ledger
        .pipe_inspect("wiki.search")
        .expect("wiki.search exists");
    assert_eq!(manifest.command_id, "wiki.search");
    assert!(!manifest.description.is_empty());
    assert!(manifest.validate_schema_hash());
}

#[test]
fn pipe_inspect_unknown_returns_command_not_found() {
    let ledger = initialized_ledger();
    let result = ledger.pipe_inspect("unknown.command");
    assert!(matches!(result, Err(seaki_pipe::CommandNotFound(ref id)) if id == "unknown.command"));
}

#[test]
fn pipe_dry_run_side_effect_free_chain() {
    let ledger = initialized_ledger();
    let ast = seaki_pipe::PipelineAst {
        pipeline_id: "dry-run-pipe".to_string(),
        steps: vec![
            seaki_pipe::PipelineStep {
                step_id: "s1".to_string(),
                command_id: "wiki.search".to_string(),
                input_binding: seaki_pipe::InputBinding::Constant(
                    serde_json::json!({"keyword": "rust"}),
                ),
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s2".to_string(),
                command_id: "citation.resolve".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s3".to_string(),
                command_id: "adr.summarize".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
        ],
    };
    let result = ledger
        .pipe_dry_run(&ast, serde_json::json!({"keyword": "rust"}))
        .expect("dry run succeeds");
    assert!(!result.events.is_empty());
    assert!(result.proposal_artifact.is_none());
    assert!(result.expected_frame_count > 0);
}

#[test]
fn pipe_dry_run_proposal_chain_outputs_artifact() {
    let ledger = initialized_ledger();
    let ast = seaki_pipe::PipelineAst {
        pipeline_id: "prop-pipe".to_string(),
        steps: vec![
            seaki_pipe::PipelineStep {
                step_id: "s1".to_string(),
                command_id: "wiki.search".to_string(),
                input_binding: seaki_pipe::InputBinding::Constant(
                    serde_json::json!({"keyword": "rust"}),
                ),
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s2".to_string(),
                command_id: "citation.resolve".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s3".to_string(),
                command_id: "wiki.patch.propose".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
        ],
    };
    let result = ledger
        .pipe_dry_run(&ast, serde_json::json!({"keyword": "rust"}))
        .expect("dry run succeeds");
    assert!(
        result.proposal_artifact.is_some(),
        "expected proposal artifact"
    );
    let artifact = result.proposal_artifact.unwrap();
    assert_eq!(artifact.patch_id, "patch-prop-pipe");
}

#[test]
fn pipe_dry_run_rejects_type_mismatch() {
    let ledger = initialized_ledger();
    let ast = seaki_pipe::PipelineAst {
        pipeline_id: "bad-pipe".to_string(),
        steps: vec![
            seaki_pipe::PipelineStep {
                step_id: "s1".to_string(),
                command_id: "wiki.search".to_string(),
                input_binding: seaki_pipe::InputBinding::Constant(
                    serde_json::json!({"keyword": "rust"}),
                ),
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s2".to_string(),
                command_id: "adr.summarize".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
        ],
    };
    let result = ledger.pipe_dry_run(&ast, serde_json::json!({"keyword": "rust"}));
    assert!(
        matches!(result, Err(CoreError::PipelineCompose(_))),
        "expected compose error, got {:?}",
        result
    );
}

#[test]
fn m1_pipe_dry_run_produces_proposal_artifact() {
    let ledger = initialized_ledger();
    let initial_events = ledger.event_count().expect("event count");

    // 1. pipe_list 验证 builtin 命令存在
    let commands = ledger.pipe_list(None);
    let ids: Vec<_> = commands.iter().map(|c| c.command_id.as_str()).collect();
    assert!(ids.contains(&"wiki.search"));
    assert!(ids.contains(&"citation.resolve"));
    assert!(ids.contains(&"wiki.patch.propose"));

    // 2. pipe_inspect 验证返回完整 manifest
    let manifest = ledger
        .pipe_inspect("wiki.search")
        .expect("wiki.search manifest");
    assert_eq!(manifest.command_id, "wiki.search");
    assert!(!manifest.description.is_empty());
    assert!(manifest.validate_schema_hash());

    // 3. 构造 PipelineAst：wiki.search -> citation.resolve -> wiki.patch.propose
    let ast = seaki_pipe::PipelineAst {
        pipeline_id: "m1-proposal-pipe".to_string(),
        steps: vec![
            seaki_pipe::PipelineStep {
                step_id: "s1".to_string(),
                command_id: "wiki.search".to_string(),
                input_binding: seaki_pipe::InputBinding::Constant(
                    serde_json::json!({"keyword": "rust"}),
                ),
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s2".to_string(),
                command_id: "citation.resolve".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
            seaki_pipe::PipelineStep {
                step_id: "s3".to_string(),
                command_id: "wiki.patch.propose".to_string(),
                input_binding: seaki_pipe::InputBinding::PreviousStep,
                failure_policy: seaki_pipe::FailurePolicy::FailFast,
            },
        ],
    };

    // 4. 调用 pipe_dry_run
    let result = ledger
        .pipe_dry_run(&ast, serde_json::json!({"keyword": "rust"}))
        .expect("dry run succeeds");

    // 5. 验证 DryRunResult
    assert!(!result.events.is_empty());
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, seaki_pipe::DryRunEvent::Request { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, seaki_pipe::DryRunEvent::StepStarted { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, seaki_pipe::DryRunEvent::Frame { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, seaki_pipe::DryRunEvent::Checkpoint { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, seaki_pipe::DryRunEvent::StepCompleted { .. })));
    assert!(result.expected_frame_count > 0);

    // 6. proposal_artifact 非空（最后一步是 proposal_only）
    let artifact = result
        .proposal_artifact
        .expect("proposal artifact should exist");
    assert_eq!(artifact.patch_id, "patch-m1-proposal-pipe");
    assert!(!artifact.diff.is_empty());

    // 7. 无实际副作用（事件数不变）
    assert_eq!(
        ledger.event_count().expect("event count"),
        initial_events,
        "dry run must not write events"
    );
}

// ---- M1 E2E: Session Search + Project Note ----
