//! Cost estimator: estimate token and compute cost for a compiled pipeline.

use crate::compiler::CompileResult;
use seaki_pipe::{Cardinality, FrameType};

/// Estimated cost for a compiled pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostEstimate {
    pub graph_id: String,
    /// Estimated CPU time in milliseconds.
    pub estimated_cpu_ms: u64,
    /// Estimated memory in megabytes.
    pub estimated_memory_mb: u64,
    /// Estimated token count (LLM steps only).
    pub estimated_tokens: u64,
    /// Confidence level: low / medium / high.
    pub confidence: CostConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostConfidence {
    Low,
    Medium,
    High,
}

/// Actual execution cost used for error-checking estimates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActualCost {
    pub cpu_ms: u64,
    pub memory_mb: u64,
    pub tokens: u64,
}

impl CostEstimate {
    /// Check whether the estimate is within an acceptable error band (0.5x ~ 2x)
    /// of the actual observed cost.
    ///
    /// # Errors
    /// Returns a descriptive error string if any metric falls outside the band.
    pub fn check_error(&self, actual: &ActualCost) -> Result<(), String> {
        let check = |label: &str, est: u64, act: u64| -> Result<(), String> {
            if act == 0 {
                return if est == 0 {
                    Ok(())
                } else {
                    Err(format!("{label}: actual is 0 but estimate is {est}"))
                };
            }
            let ratio = f64::from(est as u32) / f64::from(act as u32);
            if !(0.5..=2.0).contains(&ratio) {
                return Err(format!(
                    "{label}: estimate {est} vs actual {act} (ratio {ratio:.2} outside 0.5x~2x)"
                ));
            }
            Ok(())
        };

        check("cpu_ms", self.estimated_cpu_ms, actual.cpu_ms)?;
        check("memory_mb", self.estimated_memory_mb, actual.memory_mb)?;
        check("tokens", self.estimated_tokens, actual.tokens)?;
        Ok(())
    }
}

/// Estimator for pipeline execution cost.
///
/// Uses `PipeCommandManifest.resource_quota` as the baseline per step,
/// plus command-specific token models for LLM-heavy commands.
#[derive(Debug, Clone, Default)]
pub struct CostEstimator;

impl CostEstimator {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Estimate cost for a compiled pipeline.
    ///
    /// # Note
    /// This is a coarse estimate. Real token counts depend on input data size.
    #[must_use]
    pub fn estimate(&self, result: &CompileResult) -> CostEstimate {
        let mut cpu_ms = 0u64;
        let mut memory_mb = 0u64;
        let mut tokens = 0u64;

        for step in &result.linear_steps {
            cpu_ms += step
                .resource_quota
                .as_ref()
                .map_or(DEFAULT_CPU_MS, |q| q.cpu_ms);
            memory_mb += step
                .resource_quota
                .as_ref()
                .map_or(DEFAULT_MEMORY_MB, |q| q.memory_mb);

            if is_llm_command(&step.command_id) {
                tokens += estimate_input_tokens(&step.input_type);
                tokens += estimate_output_tokens(&step.output_type);
            }
        }

        let confidence = if result.linear_steps.len() <= 2 {
            CostConfidence::High
        } else if result.linear_steps.len() <= 5 {
            CostConfidence::Medium
        } else {
            CostConfidence::Low
        };

        CostEstimate {
            graph_id: result.graph_id.clone(),
            estimated_cpu_ms: cpu_ms,
            estimated_memory_mb: memory_mb,
            estimated_tokens: tokens,
            confidence,
        }
    }
}

const DEFAULT_CPU_MS: u64 = 100;
const DEFAULT_MEMORY_MB: u64 = 64;

fn is_llm_command(command_id: &str) -> bool {
    matches!(command_id, "adr.summarize" | "wiki.patch.propose")
}

/// Heuristic input tokens based on frame type and cardinality.
fn estimate_input_tokens(frame: &seaki_pipe::TypedFrame) -> u64 {
    match *frame {
        (FrameType::ParagraphFrame, Cardinality::One) => 256,
        (FrameType::ParagraphFrame, Cardinality::Many) => 1024,
        (FrameType::CitedParagraph, Cardinality::One) => 512,
        (FrameType::CitedParagraph, Cardinality::Many) => 2048,
        (FrameType::TextAnswer, Cardinality::One) => 128,
        (FrameType::TextAnswer, Cardinality::Many) => 512,
        (FrameType::PatchProposalArtifact, Cardinality::One) => 512,
        (FrameType::PatchProposalArtifact, Cardinality::Many) => 2048,
        (FrameType::JsonValue, Cardinality::One) => 64,
        (FrameType::JsonValue, Cardinality::Many) => 256,
    }
}

/// Heuristic output tokens based on output frame type.
fn estimate_output_tokens(frame: &seaki_pipe::TypedFrame) -> u64 {
    match frame.0 {
        FrameType::TextAnswer => 512,
        FrameType::PatchProposalArtifact => 4096,
        _ => 0,
    }
}
