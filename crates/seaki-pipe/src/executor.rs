//! Built-in command executors.

use crate::ast::{ComposedStep, FrameType};
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::run::{CommandExecutor, ExecutionContext};

/// Stub executor for `wiki.search`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WikiSearchExecutor;

impl CommandExecutor for WikiSearchExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![
            FrameEnvelope {
                seq: 1,
                step_id: step.step_id.clone(),
                frame_type: FrameType::ParagraphFrame,
                payload: serde_json::json!({
                    "paragraph_id": "para-1",
                    "text": "simulated paragraph",
                    "_simulated": true,
                    "_command": "wiki.search"
                }),
            },
            FrameEnvelope {
                seq: 2,
                step_id: step.step_id.clone(),
                frame_type: FrameType::ParagraphFrame,
                payload: serde_json::json!({
                    "paragraph_id": "para-2",
                    "text": "simulated paragraph",
                    "_simulated": true,
                    "_command": "wiki.search"
                }),
            },
        ])
    }
}

/// Stub executor for `citation.resolve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CitationResolveExecutor;

impl CommandExecutor for CitationResolveExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![
            FrameEnvelope {
                seq: 1,
                step_id: step.step_id.clone(),
                frame_type: FrameType::CitedParagraph,
                payload: serde_json::json!({
                    "citation_id": "cite-1",
                    "text": "simulated cited paragraph",
                    "_simulated": true,
                    "_command": "citation.resolve"
                }),
            },
            FrameEnvelope {
                seq: 2,
                step_id: step.step_id.clone(),
                frame_type: FrameType::CitedParagraph,
                payload: serde_json::json!({
                    "citation_id": "cite-2",
                    "text": "simulated cited paragraph",
                    "_simulated": true,
                    "_command": "citation.resolve"
                }),
            },
        ])
    }
}

/// Stub executor for `adr.summarize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdrSummarizeExecutor;

impl CommandExecutor for AdrSummarizeExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![FrameEnvelope {
            seq: 1,
            step_id: step.step_id.clone(),
            frame_type: FrameType::TextAnswer,
            payload: serde_json::json!({
                "text": "simulated answer",
                "citations": [],
                "_simulated": true,
                "_command": "adr.summarize"
            }),
        }])
    }
}

/// Stub executor for `wiki.patch.propose`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WikiPatchProposeExecutor;

impl CommandExecutor for WikiPatchProposeExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        _input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        Ok(vec![FrameEnvelope {
            seq: 1,
            step_id: step.step_id.clone(),
            frame_type: FrameType::PatchProposalArtifact,
            payload: serde_json::json!({
                "patch_id": format!("patch-{}", step.step_id),
                "diff": "simulated diff",
                "_simulated": true,
                "_command": "wiki.patch.propose"
            }),
        }])
    }
}

/// Real executor for `filter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterExecutor;

impl CommandExecutor for FilterExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        let predicate = step
            .args
            .get("predicate")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let filtered: Vec<FrameEnvelope> = input
            .into_iter()
            .filter(|frame| match &predicate {
                serde_json::Value::String(s) => frame.payload.to_string().contains(s),
                serde_json::Value::Object(pred_obj) => {
                    if let Some(frame_obj) = frame.payload.as_object() {
                        pred_obj.iter().all(|(k, v)| frame_obj.get(k) == Some(v))
                    } else {
                        false
                    }
                }
                _ => true,
            })
            .collect();

        Ok(filtered)
    }
}

/// Real executor for `map`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapExecutor;

impl CommandExecutor for MapExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError> {
        let transform = step
            .args
            .get("transform")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let mapped: Vec<FrameEnvelope> = input
            .into_iter()
            .map(|mut frame| {
                if let (Some(frame_obj), Some(trans_obj)) =
                    (frame.payload.as_object_mut(), transform.as_object())
                {
                    for (k, v) in trans_obj {
                        frame_obj.insert(k.clone(), v.clone());
                    }
                }
                frame.step_id = step.step_id.clone();
                frame.frame_type = step.output_type.0;
                frame
            })
            .collect();

        Ok(mapped)
    }
}
