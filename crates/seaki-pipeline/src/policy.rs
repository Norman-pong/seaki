//! Policy estimator: compute the aggregate capability requirements of a pipeline.

use crate::compiler::CompileResult;
use seaki_pipe::registry::SideEffectLevel;

/// Estimated policy requirements for a compiled pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEstimate {
    pub graph_id: String,
    /// Set of command IDs used in the pipeline.
    pub required_commands: Vec<String>,
    /// Maximum side-effect level across all steps.
    pub max_side_effect: SideEffectLevel,
    /// Whether the pipeline requires approval before execution.
    pub requires_approval: bool,
    /// Estimated capability grants needed.
    pub required_capabilities: Vec<String>,
}

/// Estimator for pipeline policy requirements.
#[derive(Debug, Clone, Default)]
pub struct PolicyEstimator;

impl PolicyEstimator {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Estimate policy requirements for a compiled pipeline.
    #[must_use]
    pub fn estimate(&self, result: &CompileResult) -> PolicyEstimate {
        let required_commands: Vec<String> = result
            .command_schema_hashes
            .keys()
            .cloned()
            .collect();

        let requires_approval = matches!(
            result.max_side_effect,
            SideEffectLevel::ProposalOnly | SideEffectLevel::SideEffect
        );

        let required_capabilities = required_commands
            .iter()
            .map(|cmd| format!("pipe.command.{cmd}"))
            .collect();

        PolicyEstimate {
            graph_id: result.graph_id.clone(),
            required_commands,
            max_side_effect: result.max_side_effect,
            requires_approval,
            required_capabilities,
        }
    }
}
