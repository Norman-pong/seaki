//! Real pipeline execution runtime.

use std::collections::HashMap;
use std::time::Instant;

use crate::ast::{Cardinality, ComposedPipeline, ComposedStep, FrameType, InputBinding};
use crate::dry_run::{FrameEnvelope, PipelineError};
use crate::registry::{CommandRegistry, ResourceQuota, SideEffectLevel};
use crate::ErrorKind;

/// Maximum number of frames allowed per step.
const MAX_FRAME_COUNT: u64 = 1_000;
/// Maximum frame payload size in bytes (1 MiB).
const MAX_FRAME_SIZE: u64 = 1_024 * 1_024;

pub struct ExecutionContext {
    pub workspace_id: String,
    pub actor_id: String,
    pub pipeline_id: String,
    pub audit: Vec<AuditRecord>,
    pub resource_used: ResourceUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub step_id: String,
    pub command_id: String,
    pub decision: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceUsage {
    pub cpu_ms: u64,
    pub memory_mb: u64,
    pub frame_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    pub output: Vec<FrameEnvelope>,
    pub audit: Vec<AuditRecord>,
}

/// Simplified policy decision for runtime checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    Deny,
    RequireApproval,
}

/// Simplified policy engine trait.
pub trait PolicyEngine: Send + Sync {
    /// Check whether a step is permitted to execute.
    fn check(&self, step: &ComposedStep, ctx: &ExecutionContext) -> PolicyDecision;
}

/// Placeholder policy implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimplePolicy;

impl PolicyEngine for SimplePolicy {
    fn check(&self, step: &ComposedStep, _ctx: &ExecutionContext) -> PolicyDecision {
        match step.side_effect_level {
            SideEffectLevel::None => PolicyDecision::Allow,
            _ => PolicyDecision::RequireApproval,
        }
    }
}

pub trait CommandExecutor: Send + Sync {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<FrameEnvelope>,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<FrameEnvelope>, PipelineError>;
}

/// Run a composed pipeline with real executors.
///
/// # Errors
/// Returns `PipelineError` if the pipeline is empty, a step fails and the
/// failure policy is `FailFast`, or a resource limit is exceeded.
pub fn run(
    pipeline: &ComposedPipeline,
    initial_input: serde_json::Value,
    registry: &CommandRegistry,
    executors: &HashMap<String, Box<dyn CommandExecutor>>,
    policy: &dyn PolicyEngine,
    ctx: &mut ExecutionContext,
) -> Result<RunResult, PipelineError> {
    if pipeline.steps.is_empty() {
        return Err(PipelineError {
            retryable: false,
            failed_step_id: String::new(),
            error_kind: ErrorKind::ComposeFailed,
        });
    }

    let mut step_outputs: HashMap<String, Vec<FrameEnvelope>> = HashMap::new();
    let mut previous_output: Vec<FrameEnvelope> = vec![FrameEnvelope {
        seq: 0,
        step_id: "input".to_string(),
        frame_type: pipeline.input_type.0,
        payload: initial_input,
    }];

    for step in &pipeline.steps {
        let manifest = registry
            .inspect(&step.command_id)
            .map_err(|_| PipelineError {
                retryable: false,
                failed_step_id: step.step_id.clone(),
                error_kind: ErrorKind::CommandNotFound,
            })?;

        // c. Resolve input binding into input frames.
        let input_frames = resolve_input(step, &previous_output, &step_outputs);

        // a. Check frame-level resource limits before execution.
        if let Some(quota) = &manifest.resource_quota {
            check_frame_limits(step, &input_frames)?;
            check_step_limits(step, quota, 0, ctx)?;
        }

        // b. Policy check.
        let policy_decision = policy.check(step, ctx);
        if policy_decision == PolicyDecision::Deny {
            return Err(PipelineError {
                retryable: false,
                failed_step_id: step.step_id.clone(),
                error_kind: ErrorKind::SideEffectBlocked,
            });
        }

        // d. Look up executor.
        let executor = executors
            .get(&step.command_id)
            .ok_or_else(|| PipelineError {
                retryable: false,
                failed_step_id: step.step_id.clone(),
                error_kind: ErrorKind::CommandNotFound,
            })?;

        // e. Execute.
        let start = Instant::now();
        let result = executor.execute(step, input_frames, ctx);
        let elapsed_ms = start.elapsed().as_millis() as u64;
        ctx.resource_used.cpu_ms += elapsed_ms;

        // Check step-level limits (CPU, memory) after execution.
        if let Some(quota) = &manifest.resource_quota {
            check_step_limits(step, quota, elapsed_ms, ctx)?;
        }

        // f. Handle failure policy.
        let mut output_frames = match result {
            Ok(frames) => {
                // g. Validate output frames match output_type.
                for frame in &frames {
                    if frame.frame_type != step.output_type.0 {
                        return Err(PipelineError {
                            retryable: false,
                            failed_step_id: step.step_id.clone(),
                            error_kind: ErrorKind::TypeMismatch,
                        });
                    }
                }
                // Check output frame limits.
                if let Some(_quota) = &manifest.resource_quota {
                    check_frame_limits(step, &frames)?;
                }
                ctx.resource_used.frame_count += frames.len() as u64;
                frames
            }
            Err(err) => match &step.failure_policy {
                crate::ast::FailurePolicy::FailFast => return Err(err),
                crate::ast::FailurePolicy::Skip => {
                    ctx.audit.push(AuditRecord {
                        step_id: step.step_id.clone(),
                        command_id: step.command_id.clone(),
                        decision: format!("skipped: {:?}", err.error_kind),
                        timestamp_ms: now_ms(),
                    });
                    Vec::new()
                }
                crate::ast::FailurePolicy::Default(val) => vec![FrameEnvelope {
                    seq: 0,
                    step_id: step.step_id.clone(),
                    frame_type: step.output_type.0,
                    payload: val.clone(),
                }],
            },
        };

        // Ensure output seq numbers are stable.
        for (i, frame) in output_frames.iter_mut().enumerate() {
            frame.seq = i as u64;
        }

        // h. Record audit entry for successful or skipped steps.
        if !ctx.audit.iter().any(|a| a.step_id == step.step_id) {
            ctx.audit.push(AuditRecord {
                step_id: step.step_id.clone(),
                command_id: step.command_id.clone(),
                decision: match policy_decision {
                    PolicyDecision::Allow => "allow".to_string(),
                    PolicyDecision::Deny => "deny".to_string(),
                    PolicyDecision::RequireApproval => "require_approval".to_string(),
                },
                timestamp_ms: now_ms(),
            });
        }

        // i. Update previous_output and store step output.
        previous_output = output_frames.clone();
        step_outputs.insert(step.step_id.clone(), output_frames);
    }

    Ok(RunResult {
        output: previous_output,
        audit: ctx.audit.clone(),
    })
}

fn resolve_input(
    step: &ComposedStep,
    previous_output: &[FrameEnvelope],
    step_outputs: &HashMap<String, Vec<FrameEnvelope>>,
) -> Vec<FrameEnvelope> {
    match &step.input_binding {
        InputBinding::PreviousStep => previous_output.to_vec(),
        InputBinding::Constant(val) => {
            if step.input_type.1 == Cardinality::Many && val.is_array() {
                val.as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, v)| FrameEnvelope {
                        seq: i as u64,
                        step_id: step.step_id.clone(),
                        frame_type: step.input_type.0,
                        payload: v.clone(),
                    })
                    .collect()
            } else {
                vec![FrameEnvelope {
                    seq: 0,
                    step_id: step.step_id.clone(),
                    frame_type: step.input_type.0,
                    payload: val.clone(),
                }]
            }
        }
        InputBinding::StepOutput(target_step_id) => step_outputs
            .get(target_step_id)
            .cloned()
            .unwrap_or_default(),
    }
}

fn check_frame_limits(step: &ComposedStep, frames: &[FrameEnvelope]) -> Result<(), PipelineError> {
    let frame_count = frames.len() as u64;
    if frame_count > MAX_FRAME_COUNT {
        return Err(resource_exceeded(step, "frame_count", frame_count));
    }
    for frame in frames {
        let size = serde_json::to_vec(&frame.payload)
            .map(|v| v.len() as u64)
            .unwrap_or(0);
        if size > MAX_FRAME_SIZE {
            return Err(resource_exceeded(step, "frame_size", size));
        }
    }
    Ok(())
}

fn check_step_limits(
    step: &ComposedStep,
    quota: &ResourceQuota,
    elapsed_ms: u64,
    ctx: &ExecutionContext,
) -> Result<(), PipelineError> {
    if elapsed_ms > quota.cpu_ms {
        return Err(resource_exceeded(step, "cpu_ms", elapsed_ms));
    }
    if ctx.resource_used.memory_mb > quota.memory_mb {
        return Err(resource_exceeded(
            step,
            "memory_mb",
            ctx.resource_used.memory_mb,
        ));
    }
    Ok(())
}

fn resource_exceeded(step: &ComposedStep, limit: &str, current: u64) -> PipelineError {
    PipelineError {
        retryable: false,
        failed_step_id: step.step_id.clone(),
        error_kind: ErrorKind::ResourceExceeded {
            limit: limit.to_string(),
            current,
        },
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ============================================================================
// Built-in executors
// ============================================================================

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
