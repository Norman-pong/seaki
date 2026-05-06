//! Cost estimator: estimate token and compute cost for a compiled pipeline.

use crate::compiler::CompileResult;

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

/// Estimator for pipeline execution cost.
///
/// Uses `PipeCommandManifest.resource_quota` as the baseline per step,
/// plus command-specific token multipliers for LLM-heavy commands.
#[derive(Debug, Clone, Default)]
pub struct CostEstimator {
    /// Per-command token cost multiplier for LLM-based commands.
    token_multipliers: std::collections::HashMap<String, u64>,
}

impl CostEstimator {
    #[must_use]
    pub fn new() -> Self {
        let mut token_multipliers = std::collections::HashMap::new();
        // Heuristic: summarization and patch-proposal involve LLM calls.
        token_multipliers.insert("adr.summarize".to_string(), 2048);
        token_multipliers.insert("wiki.patch.propose".to_string(), 4096);
        Self { token_multipliers }
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
            // Use resource_quota from manifest as baseline.
            // Fallback to fixed overheads if quota is not declared.
            cpu_ms += step
                .resource_quota
                .as_ref()
                .map_or(10, |q| q.cpu_ms);
            memory_mb += step
                .resource_quota
                .as_ref()
                .map_or(4, |q| q.memory_mb);

            // Add LLM token estimates for known LLM-heavy commands.
            if let Some(multiplier) = self.token_multipliers.get(&step.command_id) {
                tokens += multiplier;
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
