use crate::ast::compose;
use crate::ast::{FailurePolicy, InputBinding, PipelineAst, PipelineStep};
use crate::registry::CommandRegistry;
use crate::*;

fn composed_side_effect_free_chain() -> ComposedPipeline {
    let registry = CommandRegistry::builtin();
    let ast = PipelineAst {
        pipeline_id: "test-pipe".to_string(),
        steps: vec![
            PipelineStep {
                step_id: "s1".to_string(),
                command_id: "wiki.search".to_string(),
                input_binding: InputBinding::Constant(serde_json::json!({"keyword": "rust"})),
                failure_policy: FailurePolicy::FailFast,
            },
            PipelineStep {
                step_id: "s2".to_string(),
                command_id: "citation.resolve".to_string(),
                input_binding: InputBinding::PreviousStep,
                failure_policy: FailurePolicy::FailFast,
            },
            PipelineStep {
                step_id: "s3".to_string(),
                command_id: "adr.summarize".to_string(),
                input_binding: InputBinding::PreviousStep,
                failure_policy: FailurePolicy::FailFast,
            },
        ],
    };
    compose(&ast, &registry).expect("compose succeeds")
}

fn composed_proposal_chain() -> ComposedPipeline {
    let registry = CommandRegistry::builtin();
    let ast = PipelineAst {
        pipeline_id: "prop-pipe".to_string(),
        steps: vec![
            PipelineStep {
                step_id: "s1".to_string(),
                command_id: "wiki.search".to_string(),
                input_binding: InputBinding::Constant(serde_json::json!({"keyword": "rust"})),
                failure_policy: FailurePolicy::FailFast,
            },
            PipelineStep {
                step_id: "s2".to_string(),
                command_id: "citation.resolve".to_string(),
                input_binding: InputBinding::PreviousStep,
                failure_policy: FailurePolicy::FailFast,
            },
            PipelineStep {
                step_id: "s3".to_string(),
                command_id: "wiki.patch.propose".to_string(),
                input_binding: InputBinding::PreviousStep,
                failure_policy: FailurePolicy::FailFast,
            },
        ],
    };
    compose(&ast, &registry).expect("compose succeeds")
}

#[test]
fn dry_run_produces_events() {
    let pipeline = composed_side_effect_free_chain();
    let result = dry_run(&pipeline, serde_json::json!({"keyword": "rust"}));

    assert!(!result.events.is_empty());
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, DryRunEvent::Request { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, DryRunEvent::StepStarted { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, DryRunEvent::Frame { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, DryRunEvent::Checkpoint { .. })));
    assert!(result
        .events
        .iter()
        .any(|e| matches!(e, DryRunEvent::StepCompleted { .. })));
}

#[test]
fn dry_run_no_side_effects() {
    let pipeline = composed_side_effect_free_chain();
    let result = dry_run(&pipeline, serde_json::json!({"keyword": "rust"}));

    // All steps should be none side-effect.
    for step in &pipeline.steps {
        assert_eq!(step.side_effect_level, SideEffectLevel::None);
    }

    // No StepFailed events in a successful dry-run.
    assert!(!result
        .events
        .iter()
        .any(|e| matches!(e, DryRunEvent::StepFailed { .. })));
}

#[test]
fn dry_run_includes_proposal_artifact_when_last_step_is_proposal_only() {
    let pipeline = composed_proposal_chain();
    let result = dry_run(&pipeline, serde_json::json!({"keyword": "rust"}));

    assert!(
        result.proposal_artifact.is_some(),
        "expected proposal artifact when final step is proposal_only"
    );
    let artifact = result.proposal_artifact.unwrap();
    assert_eq!(artifact.patch_id, "patch-prop-pipe");
    assert!(!artifact.diff.is_empty());
    assert_eq!(artifact.claim_ids, vec!["claim-1", "claim-2"]);
}

#[test]
fn dry_run_no_proposal_artifact_for_none_chain() {
    let pipeline = composed_side_effect_free_chain();
    let result = dry_run(&pipeline, serde_json::json!({"keyword": "rust"}));
    assert!(result.proposal_artifact.is_none());
}

#[test]
fn dry_run_expected_frames_and_permissions() {
    let pipeline = composed_side_effect_free_chain();
    let result = dry_run(&pipeline, serde_json::json!({"keyword": "rust"}));

    // wiki.search (Many) -> 2 frames, citation.resolve (Many) -> 2 frames, adr.summarize (One) -> 1 frame = 5 frames
    assert_eq!(result.expected_frame_count, 5);
    assert!(!result.expected_permissions.is_empty());
    assert!(result
        .expected_permissions
        .contains(&"wiki:read".to_string()));
    assert!(result
        .expected_permissions
        .contains(&"citation:read".to_string()));
}

#[test]
fn dry_run_step_failed_carries_structured_error() {
    // Manually construct a failed step event.
    let error = PipelineError {
        retryable: false,
        failed_step_id: "s2".to_string(),
        error_kind: ErrorKind::CommandNotFound,
    };
    let event = DryRunEvent::StepFailed {
        step_id: "s2".to_string(),
        error: error.clone(),
    };
    match event {
        DryRunEvent::StepFailed { step_id, error } => {
            assert_eq!(step_id, "s2");
            assert_eq!(error.failed_step_id, "s2");
            assert_eq!(error.error_kind, ErrorKind::CommandNotFound);
            assert!(!error.retryable);
        }
        _ => panic!("expected StepFailed"),
    }
}
