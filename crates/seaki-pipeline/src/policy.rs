//! Policy estimator: compute the aggregate capability requirements of a pipeline.

use crate::compiler::CompileResult;
use seaki_pipe::registry::SideEffectLevel;
use std::collections::HashSet;

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
    /// Capabilities that the actor is missing.
    pub missing_capabilities: Vec<String>,
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
    ///
    /// # Arguments
    ///
    /// * `result` — The compiled pipeline result.
    /// * `actor_capabilities` — The set of capabilities the actor currently holds.
    #[must_use]
    pub fn estimate(
        &self,
        result: &CompileResult,
        actor_capabilities: &HashSet<String>,
    ) -> PolicyEstimate {
        let required_commands: Vec<String> = result
            .command_schema_hashes
            .keys()
            .cloned()
            .collect();

        let required_capabilities: Vec<String> = required_commands
            .iter()
            .map(|cmd| format!("pipe.command.{cmd}"))
            .collect();

        let missing_capabilities: Vec<String> = required_capabilities
            .iter()
            .filter(|cap| !actor_capabilities.contains(*cap))
            .cloned()
            .collect();

        let has_side_effect = matches!(
            result.max_side_effect,
            SideEffectLevel::ProposalOnly | SideEffectLevel::SideEffect
        );

        let requires_approval = has_side_effect || !missing_capabilities.is_empty();

        PolicyEstimate {
            graph_id: result.graph_id.clone(),
            required_commands,
            max_side_effect: result.max_side_effect,
            requires_approval,
            required_capabilities,
            missing_capabilities,
        }
    }
}
