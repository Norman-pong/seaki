use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;

/// A skill that the agent can dispatch to fulfill user intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillManifest {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    /// Keywords / phrases that trigger this skill.
    pub trigger_patterns: Vec<String>,
    /// Required capabilities (e.g., "file.read", "wiki.patch.propose").
    pub required_capabilities: Vec<String>,
    /// Required memory scopes (e.g., "user.preference", "project.convention").
    pub required_memory_scopes: Vec<String>,
    /// Required wiki / source scopes.
    pub required_source_scopes: Vec<String>,
    /// The pipeline template this skill generates.
    pub pipeline_template: PipelineTemplate,
    /// Admission priority (lower = higher priority).
    pub priority: u32,
    /// Whether this skill can be auto-dispatched or requires user confirmation.
    pub requires_confirmation: bool,
}

/// A pipeline template that a skill produces.
/// Template variables are denoted by `{{variable}}` in args strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineTemplate {
    pub steps: Vec<TemplateStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateStep {
    pub step_id: String,
    pub command_id: String,
    /// May contain template variables such as `"{{intent}}"` or `"{{memory.user_name}}"`.
    pub args_template: serde_json::Value,
    /// `"previous"`, `"constant"`, or `"{{step_id}}"`.
    pub input_binding: String,
}

// ---------------------------------------------------------------------------
// Skill Registry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SkillMatch {
    pub skill: SkillManifest,
    pub score: f32, // 0.0 ~ 1.0
    pub matched_pattern: String,
}

impl PartialEq for SkillMatch {
    fn eq(&self, other: &Self) -> bool {
        self.skill == other.skill
            && self.score.to_bits() == other.score.to_bits()
            && self.matched_pattern == other.matched_pattern
    }
}

#[derive(Debug, Clone)]
pub struct SkillRegistry {
    skills: HashMap<String, SkillManifest>,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Register a new skill manifest.
    ///
    /// # Errors
    /// Returns `RegistrationError` if the skill ID is duplicate, trigger patterns are empty,
    /// or any step contains an invalid command ID.
    pub fn register(&mut self, skill: SkillManifest) -> Result<(), RegistrationError> {
        if self.skills.contains_key(&skill.skill_id) {
            return Err(RegistrationError::DuplicateSkillId(skill.skill_id));
        }

        if skill.trigger_patterns.is_empty() {
            return Err(RegistrationError::EmptyTriggerPatterns(skill.skill_id));
        }

        for step in &skill.pipeline_template.steps {
            if step.command_id.trim().is_empty() {
                return Err(RegistrationError::InvalidCommandId {
                    skill_id: skill.skill_id.clone(),
                    command_id: step.command_id.clone(),
                });
            }
        }

        self.skills.insert(skill.skill_id.clone(), skill);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, skill_id: &str) -> Option<&SkillManifest> {
        self.skills.get(skill_id)
    }

    #[must_use]
    pub fn list(&self) -> Vec<&SkillManifest> {
        self.skills.values().collect()
    }

    #[must_use]
    pub fn list_by_capability(&self, capability: &str) -> Vec<&SkillManifest> {
        self.skills
            .values()
            .filter(|s| s.required_capabilities.contains(&capability.to_string()))
            .collect()
    }

    /// Match an intent string against all registered skill trigger patterns.
    ///
    /// Returns matches sorted by score descending, then by priority ascending.
    #[must_use]
    pub fn match_intent(&self, intent: &str) -> Vec<SkillMatch> {
        let intent_lower = intent.to_lowercase();
        let mut best_by_skill: HashMap<String, SkillMatch> = HashMap::new();

        for skill in self.skills.values() {
            for pattern in &skill.trigger_patterns {
                let pattern_lower = pattern.to_lowercase();
                let score = if intent_lower.contains(&pattern_lower) {
                    1.0
                } else if levenshtein_within(&intent_lower, &pattern_lower, 2) {
                    0.8
                } else {
                    continue;
                };

                let existing = best_by_skill.get(&skill.skill_id);
                if existing.is_none_or(|m| m.score < score) {
                    best_by_skill.insert(
                        skill.skill_id.clone(),
                        SkillMatch {
                            skill: skill.clone(),
                            score,
                            matched_pattern: pattern.clone(),
                        },
                    );
                }
            }
        }

        let mut result: Vec<SkillMatch> = best_by_skill.into_values().collect();
        result.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.skill.priority.cmp(&b.skill.priority))
        });
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    DuplicateSkillId(String),
    EmptyTriggerPatterns(String),
    InvalidCommandId {
        skill_id: String,
        command_id: String,
    },
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSkillId(id) => write!(f, "duplicate skill ID: {id}"),
            Self::EmptyTriggerPatterns(id) => {
                write!(f, "skill {id} has empty trigger patterns")
            }
            Self::InvalidCommandId {
                skill_id,
                command_id,
            } => write!(f, "skill {skill_id} has invalid command ID: {command_id}"),
        }
    }
}

impl std::error::Error for RegistrationError {}

// ---------------------------------------------------------------------------
// Bounded Levenshtein distance
// ---------------------------------------------------------------------------

fn levenshtein_within(a: &str, b: &str, max: usize) -> bool {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len.abs_diff(b_len) > max {
        return false;
    }

    let mut prev = vec![0; b_len + 1];
    let mut curr = vec![0; b_len + 1];

    for (j, item) in prev.iter_mut().enumerate().take(b_len + 1) {
        *item = j;
    }

    for i in 1..=a_len {
        curr[0] = i;
        let mut row_min = curr[0];

        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            row_min = row_min.min(curr[j]);
        }

        if row_min > max {
            return false;
        }

        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len] <= max
}

// ---------------------------------------------------------------------------
// Skill Admission
// ---------------------------------------------------------------------------

/// Validates whether a skill can be dispatched given current actor capabilities and memory.
#[derive(Debug, Clone, Copy)]
pub struct SkillAdmission;

impl SkillAdmission {
    /// Check whether a skill is admissible for the given actor and workspace.
    ///
    /// # Errors
    /// Returns `AdmissionError::CapabilityCheckFailed` if the capability store is poisoned
    /// or otherwise returns an error during grant lookup.
    pub fn check(
        skill: &SkillManifest,
        capability_store: &seaki_policy::CapabilityStore,
        actor_id: &str,
        workspace_id: &str,
    ) -> Result<AdmissionCheck, AdmissionError> {
        let mut missing_capabilities = Vec::new();
        let now = SystemTime::now();

        for capability in &skill.required_capabilities {
            match capability_store.has_valid_generic_grant(
                actor_id,
                workspace_id,
                capability,
                "execute",
                "agent",
                now,
            ) {
                Ok(true) => {}
                Ok(false) => {
                    missing_capabilities.push(capability.clone());
                }
                Err(e) => {
                    return Err(AdmissionError::CapabilityCheckFailed(e.to_string()));
                }
            }
        }

        let mut missing_memory_scopes = Vec::new();
        for scope in &skill.required_memory_scopes {
            match capability_store.has_memory_scope(actor_id, workspace_id, scope) {
                Ok(true) => {}
                Ok(false) => {
                    missing_memory_scopes.push(scope.clone());
                }
                Err(e) => {
                    return Err(AdmissionError::MemoryScopeCheckFailed(e.to_string()));
                }
            }
        }

        let mut missing_source_scopes = Vec::new();
        for scope in &skill.required_source_scopes {
            match capability_store.has_source_scope(actor_id, workspace_id, scope) {
                Ok(true) => {}
                Ok(false) => {
                    missing_source_scopes.push(scope.clone());
                }
                Err(e) => {
                    return Err(AdmissionError::SourceScopeCheckFailed(e.to_string()));
                }
            }
        }

        let allowed = missing_capabilities.is_empty()
            && missing_memory_scopes.is_empty()
            && missing_source_scopes.is_empty();

        Ok(AdmissionCheck {
            skill_id: skill.skill_id.clone(),
            allowed,
            missing_capabilities,
            missing_memory_scopes,
            missing_source_scopes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionCheck {
    pub skill_id: String,
    pub allowed: bool,
    pub missing_capabilities: Vec<String>,
    pub missing_memory_scopes: Vec<String>,
    pub missing_source_scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    CapabilityCheckFailed(String),
    MemoryScopeCheckFailed(String),
    SourceScopeCheckFailed(String),
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityCheckFailed(msg) => {
                write!(f, "capability check failed: {msg}")
            }
            Self::MemoryScopeCheckFailed(msg) => {
                write!(f, "memory scope check failed: {msg}")
            }
            Self::SourceScopeCheckFailed(msg) => {
                write!(f, "source scope check failed: {msg}")
            }
        }
    }
}

impl std::error::Error for AdmissionError {}
