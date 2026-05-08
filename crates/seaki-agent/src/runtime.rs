//! Agent runtime: full intent execution loop with skill dispatch, pipeline
//! compose, dry-run, approval gate, and run.

use std::collections::HashMap;

use crate::session::{SessionState, SessionStateMachine};
use seaki_pipe::approval_gate::{ApprovalGate, ApprovalRequestInput};
use seaki_pipe::compose;
use seaki_pipe::dry_run::{dry_run, DryRunResult, FrameEnvelope};
use seaki_pipe::registry::CommandRegistry;
use seaki_pipe::run::{
    run, run_resume, AuditRecord, CommandExecutor, ExecutionContext, ResourceUsage, StepPolicy,
};
use seaki_policy::ApprovalStatus;

use crate::dispatch::{DispatchError, SkillDispatcher};
use crate::llm::{LlmClient, LlmError, LlmMessage, LlmRequest, MessageRole};
use crate::session::Session;
use crate::skill::SkillRegistry;

/// Context passed to the agent for pipeline proposal generation.
pub struct AgentContext {
    pub workspace_id: String,
    pub actor_id: String,
    pub session_id: Option<String>,
}

/// Result of a successful agent intent execution.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentResponse {
    pub answer: String,
    pub citations: Vec<String>,
    pub pipeline_id: String,
    pub audit_trail: Vec<AuditRecord>,
    pub dry_run_result: Option<DryRunResult>,
    pub approval_required: bool,
}

/// Errors that can occur during agent intent execution.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentExecutionError {
    NoMatchingSkill,
    SkillNotAllowed { skill_id: String, reason: String },
    PipelineCompileFailed(String),
    DryRunFailed(String),
    ApprovalRequired { step_id: String, operation: String },
    ApprovalDenied,
    ExecutionFailed(String),
    LlmError(LlmError),
}

impl std::fmt::Display for AgentExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatchingSkill => write!(f, "no matching skill found for intent"),
            Self::SkillNotAllowed { skill_id, reason } => {
                write!(f, "skill {skill_id} not allowed: {reason}")
            }
            Self::PipelineCompileFailed(msg) => write!(f, "pipeline compile failed: {msg}"),
            Self::DryRunFailed(msg) => write!(f, "dry run failed: {msg}"),
            Self::ApprovalRequired { step_id, operation } => {
                write!(f, "approval required for step {step_id}: {operation}")
            }
            Self::ApprovalDenied => write!(f, "approval denied"),
            Self::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
            Self::LlmError(e) => write!(f, "LLM error: {e}"),
        }
    }
}

impl std::error::Error for AgentExecutionError {}

/// Extended AgentRuntime with full execution loop.
pub struct AgentRuntime {
    pub llm: Box<dyn LlmClient>,
    pub skill_dispatcher: SkillDispatcher,
    pub command_registry: CommandRegistry,
}

impl AgentRuntime {
    /// Create a new runtime with only an LLM client.
    ///
    /// Backward-compatible constructor; skill dispatcher and command registry
    /// start empty.
    pub fn new(llm: Box<dyn LlmClient>) -> Self {
        Self {
            llm,
            skill_dispatcher: SkillDispatcher::new(SkillRegistry::new()),
            command_registry: CommandRegistry::new(),
        }
    }

    /// Create a new runtime with the given components.
    pub fn with_components(
        llm: Box<dyn LlmClient>,
        skill_dispatcher: SkillDispatcher,
        command_registry: CommandRegistry,
    ) -> Self {
        Self {
            llm,
            skill_dispatcher,
            command_registry,
        }
    }

    /// Execute a user intent through the full pipeline:
    /// dispatch → compile → dry-run → approval → execute → answer.
    ///
    /// # Errors
    /// Returns `AgentExecutionError` at any stage of the pipeline.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_intent(
        &self,
        intent: &str,
        session: &mut Session,
        state_machine: &mut SessionStateMachine,
        capability_store: &seaki_policy::CapabilityStore,
        gate: &dyn ApprovalGate,
        policy: &dyn StepPolicy,
        executors: &HashMap<String, Box<dyn CommandExecutor>>,
    ) -> Result<AgentResponse, AgentExecutionError> {
        if session.messages.is_empty() {
            return Err(AgentExecutionError::ExecutionFailed(
                "session has no messages".to_string(),
            ));
        }

        // 1. Idle → Planning
        state_machine
            .transition(SessionState::Planning, "starting intent execution")
            .map_err(|e| AgentExecutionError::ExecutionFailed(e.to_string()))?;

        // 2. Dispatch
        let dispatch_result = self
            .skill_dispatcher
            .dispatch(intent, session, capability_store, &self.command_registry)
            .map_err(|e| match e {
                DispatchError::NoMatchingSkill => AgentExecutionError::NoMatchingSkill,
                DispatchError::SkillNotAllowed { skill_id, reason } => {
                    AgentExecutionError::SkillNotAllowed { skill_id, reason }
                }
                DispatchError::PipelineRenderFailed { skill_id, reason } => {
                    AgentExecutionError::PipelineCompileFailed(format!(
                        "skill {skill_id}: {reason}"
                    ))
                }
                DispatchError::CommandNotFound { command_id } => {
                    AgentExecutionError::PipelineCompileFailed(format!(
                        "command not found: {command_id}"
                    ))
                }
            })?;

        let pipeline_ast = dispatch_result.pipeline;

        // 3. Compose
        let composed = compose(&pipeline_ast, &self.command_registry)
            .map_err(|e| AgentExecutionError::PipelineCompileFailed(e.to_string()))?;

        // 4. Dry run
        let initial_input = serde_json::json!({ "intent": intent });
        let dry_run_result = dry_run(&composed, initial_input.clone());

        // 5. Planning → Executing
        state_machine
            .transition(
                SessionState::Executing,
                "pipeline composed, starting execution",
            )
            .map_err(|e| AgentExecutionError::ExecutionFailed(e.to_string()))?;

        // 6. Execute
        let mut ctx = ExecutionContext {
            workspace_id: session.workspace_id.clone(),
            actor_id: session.actor_id.clone(),
            pipeline_id: composed.pipeline_id.clone(),
            audit: Vec::new(),
            resource_used: ResourceUsage::default(),
            checkpoint_outputs: HashMap::new(),
        };

        let run_result = run(
            &composed,
            initial_input.clone(),
            &self.command_registry,
            executors,
            policy,
            &mut ctx,
        );

        let (output, audit, approval_required) = match run_result {
            Ok(result) => (result.output, result.audit, false),
            Err(seaki_pipe::dry_run::PipelineError {
                error_kind: seaki_pipe::dry_run::ErrorKind::ApprovalRequired,
                failed_step_id,
                ..
            }) => {
                let approval_id = gate
                    .request_approval(ApprovalRequestInput {
                        pipeline_id: composed.pipeline_id.clone(),
                        step_id: failed_step_id.clone(),
                        actor_id: session.actor_id.clone(),
                        workspace_id: session.workspace_id.clone(),
                        operation: format!("execute step {failed_step_id}"),
                        reason: "side-effect approval required".to_string(),
                    })
                    .map_err(|e| AgentExecutionError::ExecutionFailed(e.to_string()))?;

                let status = gate
                    .wait_for_approval(&approval_id, 5000)
                    .map_err(|e| AgentExecutionError::ExecutionFailed(e.to_string()))?;

                match status {
                    ApprovalStatus::Approved => {
                        let checkpoint_outputs = std::mem::take(&mut ctx.checkpoint_outputs);
                        let mut retry_ctx = ExecutionContext {
                            workspace_id: session.workspace_id.clone(),
                            actor_id: session.actor_id.clone(),
                            pipeline_id: composed.pipeline_id.clone(),
                            audit: std::mem::take(&mut ctx.audit),
                            resource_used: std::mem::take(&mut ctx.resource_used),
                            checkpoint_outputs: HashMap::new(),
                        };

                        let retry_result = run_resume(
                            &composed,
                            initial_input,
                            &self.command_registry,
                            executors,
                            policy,
                            &mut retry_ctx,
                            &failed_step_id,
                            &checkpoint_outputs,
                        );

                        match retry_result {
                            Ok(result) => (result.output, retry_ctx.audit, true),
                            Err(e) => {
                                return Err(AgentExecutionError::ExecutionFailed(format!(
                                    "retry failed: {:?}",
                                    e.error_kind
                                )))
                            }
                        }
                    }
                    ApprovalStatus::Denied => return Err(AgentExecutionError::ApprovalDenied),
                    ApprovalStatus::Pending => {
                        return Err(AgentExecutionError::ExecutionFailed(
                            "approval still pending".to_string(),
                        ))
                    }
                }
            }
            Err(e) => {
                return Err(AgentExecutionError::ExecutionFailed(format!(
                    "{:?}",
                    e.error_kind
                )))
            }
        };

        // 7. Extract answer
        let (answer, citations) = extract_answer(&output);

        // 8. Executing → Idle
        state_machine
            .transition(SessionState::Idle, "execution completed")
            .map_err(|e| AgentExecutionError::ExecutionFailed(e.to_string()))?;

        Ok(AgentResponse {
            answer,
            citations,
            pipeline_id: composed.pipeline_id,
            audit_trail: audit,
            dry_run_result: Some(dry_run_result),
            approval_required,
        })
    }

    /// Generate a pipeline proposal from user intent.
    pub fn propose_pipeline(
        &self,
        intent: &str,
        _context: &AgentContext,
    ) -> Result<serde_json::Value, LlmError> {
        let request = LlmRequest {
            model: "mock".to_string(),
            messages: vec![
                LlmMessage {
                    role: MessageRole::System,
                    content: "You are a pipeline designer.".to_string(),
                    name: None,
                },
                LlmMessage {
                    role: MessageRole::User,
                    content: intent.to_string(),
                    name: None,
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(2048),
        };
        let response = self.llm.complete(request)?;
        serde_json::from_str(&response.content).map_err(|e| LlmError::ParseFailed(e.to_string()))
    }
}

fn extract_answer(output: &[FrameEnvelope]) -> (String, Vec<String>) {
    if let Some(frame) = output.last() {
        match frame.frame_type {
            seaki_pipe::ast::FrameType::TextAnswer => {
                let text = frame
                    .payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let citations = frame
                    .payload
                    .get("citations")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                (text, citations)
            }
            _ => {
                let text = frame.payload.to_string();
                (text, Vec::new())
            }
        }
    } else {
        (String::new(), Vec::new())
    }
}

/// Builder for constructing an AgentRuntime with skills and commands.
pub struct AgentRuntimeBuilder {
    llm: Option<Box<dyn LlmClient>>,
    skill_registry: SkillRegistry,
    command_registry: CommandRegistry,
}

impl std::fmt::Debug for AgentRuntimeBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRuntimeBuilder")
            .field("llm", &self.llm.is_some())
            .field("skill_registry", &self.skill_registry)
            .field("command_registry", &self.command_registry)
            .finish()
    }
}

impl AgentRuntimeBuilder {
    /// Create a new builder with empty registries.
    pub fn new() -> Self {
        Self {
            llm: None,
            skill_registry: SkillRegistry::new(),
            command_registry: CommandRegistry::new(),
        }
    }

    /// Set the LLM client.
    pub fn with_llm(mut self, llm: Box<dyn LlmClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Set the skill registry.
    pub fn with_skill_registry(mut self, registry: SkillRegistry) -> Self {
        self.skill_registry = registry;
        self
    }

    /// Set the command registry.
    pub fn with_command_registry(mut self, registry: CommandRegistry) -> Self {
        self.command_registry = registry;
        self
    }

    /// Build the [`AgentRuntime`].
    ///
    /// # Errors
    /// Returns an error if the LLM client was not provided.
    pub fn build(self) -> Result<AgentRuntime, String> {
        let llm = self.llm.ok_or("LLM client is required")?;
        let skill_dispatcher = SkillDispatcher::new(self.skill_registry);
        Ok(AgentRuntime {
            llm,
            skill_dispatcher,
            command_registry: self.command_registry,
        })
    }
}

impl Default for AgentRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
