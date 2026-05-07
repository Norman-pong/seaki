use crate::session::Session;
use crate::skill::{SkillAdmission, SkillRegistry};
use seaki_pipe::ast::{FailurePolicy, InputBinding, PipelineAst, PipelineStep};
use seaki_pipe::registry::CommandRegistry;
use seaki_policy::CapabilityStore;

/// Context injected into the pipeline from the current session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectedContext {
    pub memory_items: Vec<String>,
    pub wiki_claims: Vec<String>,
    pub session_summary: String,
}

/// Result of a successful skill dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchResult {
    pub skill_id: String,
    pub skill_name: String,
    pub pipeline: PipelineAst,
    pub requires_confirmation: bool,
    pub admission_check: crate::skill::AdmissionCheck,
    pub injected_context: InjectedContext,
}

/// Errors that can occur during skill dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchError {
    NoMatchingSkill,
    SkillNotAllowed { skill_id: String, reason: String },
    PipelineRenderFailed { skill_id: String, reason: String },
    CommandNotFound { command_id: String },
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatchingSkill => write!(f, "no matching skill found for intent"),
            Self::SkillNotAllowed { skill_id, reason } => {
                write!(f, "skill {skill_id} not allowed: {reason}")
            }
            Self::PipelineRenderFailed { skill_id, reason } => {
                write!(f, "pipeline render failed for skill {skill_id}: {reason}")
            }
            Self::CommandNotFound { command_id } => {
                write!(f, "command not found: {command_id}")
            }
        }
    }
}

impl std::error::Error for DispatchError {}

/// Dispatches user intent to a skill and produces an executable pipeline.
pub struct SkillDispatcher {
    pub registry: SkillRegistry,
    pub admission: SkillAdmission,
}

impl SkillDispatcher {
    #[must_use]
    pub fn new(registry: SkillRegistry) -> Self {
        Self {
            registry,
            admission: SkillAdmission,
        }
    }

    /// Main dispatch entry: match intent → check admission → render pipeline.
    ///
    /// # Errors
    /// Returns `DispatchError` if no skill matches, admission fails, or pipeline rendering fails.
    pub fn dispatch(
        &self,
        intent: &str,
        session: &Session,
        capability_store: &CapabilityStore,
        command_registry: &CommandRegistry,
    ) -> Result<DispatchResult, DispatchError> {
        // 1. Match intent against registry.
        let matches = self.registry.match_intent(intent);
        if matches.is_empty() {
            return Err(DispatchError::NoMatchingSkill);
        }

        // 2. Take the highest-scoring match.
        let skill_match = &matches[0];
        let skill = &skill_match.skill;

        // 3. Check admission.
        let admission_check = SkillAdmission::check(
            skill,
            capability_store,
            &session.actor_id,
            &session.workspace_id,
        )
        .map_err(|e| DispatchError::SkillNotAllowed {
            skill_id: skill.skill_id.clone(),
            reason: e.to_string(),
        })?;

        if !admission_check.allowed {
            let missing = admission_check.missing_capabilities.join(", ");
            let reason = if missing.is_empty() {
                "missing required scopes".to_string()
            } else {
                format!("missing capabilities: {missing}")
            };
            return Err(DispatchError::SkillNotAllowed {
                skill_id: skill.skill_id.clone(),
                reason,
            });
        }

        // 4. Build injected context.
        let injected_context = build_injected_context(session);

        // 5. Render pipeline template.
        let pipeline = render_pipeline(
            &skill.pipeline_template,
            intent,
            &injected_context,
            command_registry,
        )
        .map_err(|e| match e {
            DispatchError::CommandNotFound { command_id } => {
                DispatchError::CommandNotFound { command_id }
            }
            other => DispatchError::PipelineRenderFailed {
                skill_id: skill.skill_id.clone(),
                reason: other.to_string(),
            },
        })?;

        Ok(DispatchResult {
            skill_id: skill.skill_id.clone(),
            skill_name: skill.name.clone(),
            pipeline,
            requires_confirmation: skill.requires_confirmation,
            admission_check,
            injected_context,
        })
    }
}

fn build_injected_context(session: &Session) -> InjectedContext {
    // memory_items: first 5 claims.
    let memory_items: Vec<String> = session
        .claims
        .iter()
        .take(5)
        .map(|c| c.text.clone())
        .collect();

    // wiki_claims: last 5 claims.
    let wiki_claims: Vec<String> = session
        .claims
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(|c| c.text.clone())
        .collect();

    // session_summary: first 3 user messages, truncated to 200 chars.
    let user_messages: Vec<&crate::session::SessionMessage> = session
        .messages
        .iter()
        .filter(|m| matches!(m.role, crate::llm::MessageRole::User))
        .take(3)
        .collect();

    let summary_raw = user_messages
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let session_summary = if summary_raw.len() > 200 {
        summary_raw[..200].to_string()
    } else {
        summary_raw
    };

    InjectedContext {
        memory_items,
        wiki_claims,
        session_summary,
    }
}

fn render_pipeline(
    template: &crate::skill::PipelineTemplate,
    intent: &str,
    context: &InjectedContext,
    command_registry: &CommandRegistry,
) -> Result<PipelineAst, DispatchError> {
    // 1. Validate all command_ids exist.
    for step in &template.steps {
        if command_registry.inspect(&step.command_id).is_err() {
            return Err(DispatchError::CommandNotFound {
                command_id: step.command_id.clone(),
            });
        }
    }

    // 2. Render each step.
    let mut steps = Vec::with_capacity(template.steps.len());
    for template_step in &template.steps {
        let args = substitute_in_value(&template_step.args_template, intent, context);
        let input_binding = parse_input_binding(&template_step.input_binding);

        steps.push(PipelineStep {
            step_id: template_step.step_id.clone(),
            command_id: template_step.command_id.clone(),
            input_binding,
            failure_policy: FailurePolicy::FailFast,
            args,
        });
    }

    // Generate a deterministic pipeline ID.
    let pipeline_id = format!(
        "pipeline:{}",
        template.steps.first().map_or("empty", |s| &s.step_id)
    );

    Ok(PipelineAst { pipeline_id, steps })
}

fn parse_input_binding(binding: &str) -> InputBinding {
    match binding {
        "previous" => InputBinding::PreviousStep,
        "constant" => InputBinding::Constant(serde_json::Value::Null),
        other => {
            let trimmed = other.trim();
            if trimmed.starts_with("{{") && trimmed.ends_with("}}") {
                let inner = &trimmed[2..trimmed.len() - 2];
                InputBinding::StepOutput(inner.to_string())
            } else {
                // Default fallback per spec.
                InputBinding::PreviousStep
            }
        }
    }
}

fn substitute_in_value(
    value: &serde_json::Value,
    intent: &str,
    context: &InjectedContext,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            serde_json::Value::String(substitute_vars(s, intent, context))
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| substitute_in_value(v, intent, context))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute_in_value(v, intent, context)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn substitute_vars(template: &str, intent: &str, context: &InjectedContext) -> String {
    let mut result = template.to_string();

    // {{intent}}
    result = result.replace("{{intent}}", intent);

    // {{session.summary}}
    result = result.replace("{{session.summary}}", &context.session_summary);

    // {{memory.N}}
    for (i, item) in context.memory_items.iter().enumerate() {
        result = result.replace(&format!("{{{{memory.{i}}}}}",), item);
    }

    // {{wiki.N}}
    for (i, item) in context.wiki_claims.iter().enumerate() {
        result = result.replace(&format!("{{{{wiki.{i}}}}}",), item);
    }

    // Remove any remaining unmatched template variables.
    // This is a best-effort cleanup for simple {{var}} patterns.
    loop {
        let mut changed = false;
        if let Some(start) = result.find("{{") {
            if let Some(end) = result[start..].find("}}") {
                let end = start + end + 2;
                result.replace_range(start..end, "");
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    result
}
