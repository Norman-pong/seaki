//! Pipeline dry-run: JSONL event stream, checkpoint, proposal artifact.

use crate::ast::{ComposedPipeline, ComposedStep, FrameType};
use crate::registry::SideEffectLevel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DryRunEvent {
    Request {
        pipeline_id: String,
        input: serde_json::Value,
    },
    StepStarted {
        step_id: String,
    },
    Frame {
        step_id: String,
        envelope: FrameEnvelope,
    },
    Checkpoint {
        step_id: String,
        input_hash: String,
        output_hash: String,
        frame_offset: u64,
    },
    StepCompleted {
        step_id: String,
    },
    StepFailed {
        step_id: String,
        error: PipelineError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameEnvelope {
    pub seq: u64,
    pub step_id: String,
    pub frame_type: FrameType,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineError {
    pub retryable: bool,
    pub failed_step_id: String,
    pub error_kind: ErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    TypeMismatch,
    CommandNotFound,
    SchemaValidationFailed,
    QuotaExceeded,
    ComposeFailed,
    SideEffectBlocked,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TypeMismatch => f.write_str("TypeMismatch"),
            Self::CommandNotFound => f.write_str("CommandNotFound"),
            Self::SchemaValidationFailed => f.write_str("SchemaValidationFailed"),
            Self::QuotaExceeded => f.write_str("QuotaExceeded"),
            Self::ComposeFailed => f.write_str("ComposeFailed"),
            Self::SideEffectBlocked => f.write_str("SideEffectBlocked"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchProposalArtifact {
    pub patch_id: String,
    pub base_revision: String,
    pub diff: String,
    pub claim_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryRunResult {
    pub events: Vec<DryRunEvent>,
    pub expected_read_ranges: Vec<String>,
    pub expected_permissions: Vec<String>,
    pub expected_frame_count: u64,
    pub proposal_artifact: Option<PatchProposalArtifact>,
}

/// Simulate a dry-run of a composed pipeline without executing actual side effects.
pub fn dry_run(pipeline: &ComposedPipeline, initial_input: serde_json::Value) -> DryRunResult {
    let mut events = Vec::new();
    let mut expected_read_ranges = Vec::new();
    let mut expected_permissions = Vec::new();
    let mut frame_count: u64 = 0;
    let mut seq: u64 = 0;

    events.push(DryRunEvent::Request {
        pipeline_id: pipeline.pipeline_id.clone(),
        input: initial_input.clone(),
    });

    let mut current_payload = initial_input;

    for step in &pipeline.steps {
        events.push(DryRunEvent::StepStarted {
            step_id: step.step_id.clone(),
        });

        // Simulate read ranges and permissions based on command.
        match step.command_id.as_str() {
            "wiki.search" => {
                expected_read_ranges.push(format!("wiki:index:{}", step.step_id));
                expected_permissions.push("wiki:read".to_string());
            }
            "citation.resolve" => {
                expected_read_ranges.push(format!("citation:index:{}", step.step_id));
                expected_permissions.push("citation:read".to_string());
            }
            "adr.summarize" => {
                expected_read_ranges.push(format!("adr:corpus:{}", step.step_id));
                expected_permissions.push("adr:read".to_string());
            }
            "wiki.patch.propose" => {
                expected_read_ranges.push(format!("wiki:draft:{}", step.step_id));
                expected_permissions.push("wiki:propose".to_string());
            }
            _ => {}
        }

        // Simulate output frames based on cardinality.
        let output_frames = match step.output_type.1 {
            crate::ast::Cardinality::One => vec![simulate_frame(step, &mut seq)],
            crate::ast::Cardinality::Many => {
                // Simulate 2 frames for Many cardinality.
                vec![
                    simulate_frame(step, &mut seq),
                    simulate_frame(step, &mut seq),
                ]
            }
        };

        let input_hash = hash_value(&current_payload);
        for (offset, frame) in output_frames.iter().enumerate() {
            frame_count += 1;
            events.push(DryRunEvent::Frame {
                step_id: step.step_id.clone(),
                envelope: frame.clone(),
            });
            events.push(DryRunEvent::Checkpoint {
                step_id: step.step_id.clone(),
                input_hash: input_hash.clone(),
                output_hash: hash_value(&frame.payload),
                frame_offset: offset as u64,
            });
        }

        // Update current payload to the last frame's payload for next step.
        if let Some(last) = output_frames.last() {
            current_payload = last.payload.clone();
        }

        events.push(DryRunEvent::StepCompleted {
            step_id: step.step_id.clone(),
        });
    }

    // If the final step is proposal_only, generate a PatchProposalArtifact.
    let proposal_artifact = pipeline.steps.last().and_then(|last_step| {
        if last_step.side_effect_level == SideEffectLevel::ProposalOnly {
            Some(PatchProposalArtifact {
                patch_id: format!("patch-{}", pipeline.pipeline_id),
                base_revision: "1".to_string(),
                diff: format!(
                    "// simulated diff for {}\n+ proposed change",
                    last_step.step_id
                ),
                claim_ids: vec!["claim-1".to_string(), "claim-2".to_string()],
            })
        } else {
            None
        }
    });

    DryRunResult {
        events,
        expected_read_ranges,
        expected_permissions,
        expected_frame_count: frame_count,
        proposal_artifact,
    }
}

fn simulate_frame(step: &ComposedStep, seq: &mut u64) -> FrameEnvelope {
    *seq += 1;
    let payload = match step.output_type.0 {
        FrameType::ParagraphFrame => serde_json::json!({
            "paragraph_id": format!("para-{}", seq),
            "text": "simulated paragraph"
        }),
        FrameType::CitedParagraph => serde_json::json!({
            "citation_id": format!("cite-{}", seq),
            "text": "simulated cited paragraph"
        }),
        FrameType::TextAnswer => serde_json::json!({
            "text": "simulated answer",
            "citations": []
        }),
        FrameType::PatchProposalArtifact => serde_json::json!({
            "patch_id": format!("patch-{}", seq),
            "diff": "simulated diff"
        }),
        FrameType::JsonValue => serde_json::json!({"seq": *seq}),
    };

    FrameEnvelope {
        seq: *seq,
        step_id: step.step_id.clone(),
        frame_type: step.output_type.0,
        payload,
    }
}

fn hash_value(value: &serde_json::Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_string(value).unwrap_or_default().as_bytes());
    hex_digest(hasher.finalize().as_slice())
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::compose;
    use crate::ast::{FailurePolicy, InputBinding, PipelineAst, PipelineStep};
    use crate::registry::CommandRegistry;

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
}
