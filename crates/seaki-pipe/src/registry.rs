//! Pipe command registry: manifest validation, inspect, list.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceQuota {
    pub cpu_ms: u64,
    pub memory_mb: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideEffectLevel {
    None,
    ProposalOnly,
    SideEffect,
}

impl SideEffectLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ProposalOnly => "proposal_only",
            Self::SideEffect => "side_effect",
        }
    }
}

impl std::fmt::Display for SideEffectLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SideEffectLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "proposal_only" => Ok(Self::ProposalOnly),
            "side_effect" => Ok(Self::SideEffect),
            other => Err(format!("unknown side_effect_level: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeCommandManifest {
    pub command_id: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub side_effect_level: SideEffectLevel,
    pub resource_quota: Option<ResourceQuota>,
    pub schema_hash: String,
}

impl PipeCommandManifest {
    /// Compute the canonical schema hash from `input_schema` and `output_schema`.
    #[must_use]
    pub fn compute_schema_hash(
        input_schema: &serde_json::Value,
        output_schema: &serde_json::Value,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"input:");
        hasher.update(
            serde_json::to_string(input_schema)
                .expect("json serialization")
                .as_bytes(),
        );
        hasher.update(b";output:");
        hasher.update(
            serde_json::to_string(output_schema)
                .expect("json serialization")
                .as_bytes(),
        );
        hex_digest(hasher.finalize().as_slice())
    }

    #[must_use]
    pub fn validate_schema_hash(&self) -> bool {
        let expected = Self::compute_schema_hash(&self.input_schema, &self.output_schema);
        self.schema_hash == expected
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    SchemaHashMismatch { expected: String, found: String },
    DuplicateCommandId(String),
    InvalidCommandId(String),
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaHashMismatch { expected, found } => {
                write!(
                    f,
                    "schema hash mismatch: expected {expected}, found {found}"
                )
            }
            Self::DuplicateCommandId(id) => write!(f, "duplicate command id: {id}"),
            Self::InvalidCommandId(id) => write!(f, "invalid command id: {id}"),
        }
    }
}

impl std::error::Error for RegistrationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandNotFound(pub String);

impl std::fmt::Display for CommandNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "command not found: {}", self.0)
    }
}

impl std::error::Error for CommandNotFound {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRegistry {
    commands: HashMap<String, PipeCommandManifest>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    #[must_use]
    pub fn builtin() -> Self {
        let mut registry = Self::new();
        for manifest in builtin_commands() {
            registry
                .register(manifest)
                .expect("builtin commands are valid");
        }
        registry
    }

    pub fn register(&mut self, manifest: PipeCommandManifest) -> Result<(), RegistrationError> {
        if manifest.command_id.trim().is_empty() {
            return Err(RegistrationError::InvalidCommandId(manifest.command_id));
        }

        let expected_hash = PipeCommandManifest::compute_schema_hash(
            &manifest.input_schema,
            &manifest.output_schema,
        );
        if manifest.schema_hash != expected_hash {
            return Err(RegistrationError::SchemaHashMismatch {
                expected: expected_hash,
                found: manifest.schema_hash.clone(),
            });
        }

        if self.commands.contains_key(&manifest.command_id) {
            return Err(RegistrationError::DuplicateCommandId(
                manifest.command_id.clone(),
            ));
        }

        self.commands.insert(manifest.command_id.clone(), manifest);
        Ok(())
    }

    pub fn inspect(&self, command_id: &str) -> Result<&PipeCommandManifest, CommandNotFound> {
        self.commands
            .get(command_id)
            .ok_or_else(|| CommandNotFound(command_id.to_string()))
    }

    #[must_use]
    pub fn list(&self) -> Vec<&PipeCommandManifest> {
        let mut manifests: Vec<_> = self.commands.values().collect();
        manifests.sort_by_key(|m| &m.command_id);
        manifests
    }

    #[must_use]
    pub fn list_by_side_effect(&self, level: SideEffectLevel) -> Vec<&PipeCommandManifest> {
        self.commands
            .values()
            .filter(|m| m.side_effect_level == level)
            .collect::<Vec<_>>()
            .into_iter()
            .collect()
    }
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

#[allow(clippy::too_many_lines)]
fn builtin_commands() -> Vec<PipeCommandManifest> {
    let input_keyword = serde_json::json!({
        "type": "object",
        "properties": {
            "keyword": { "type": "string" }
        },
        "required": ["keyword"]
    });
    let output_paragraph_frames = serde_json::json!({
        "type": "array",
        "items": { "$ref": "#/definitions/ParagraphFrame" }
    });
    let input_cited_paragraphs = serde_json::json!({
        "type": "array",
        "items": { "$ref": "#/definitions/CitedParagraph" }
    });
    let output_text_answer = serde_json::json!({
        "type": "object",
        "properties": {
            "text": { "type": "string" },
            "citations": { "type": "array" }
        },
        "required": ["text"]
    });
    let output_patch_proposal = serde_json::json!({
        "type": "object",
        "properties": {
            "patch_id": { "type": "string" },
            "diff": { "type": "string" }
        },
        "required": ["patch_id", "diff"]
    });
    let passthrough_array = serde_json::json!({
        "type": "array"
    });
    let _passthrough_object = serde_json::json!({
        "type": "object"
    });

    vec![
        PipeCommandManifest {
            command_id: "wiki.search".to_string(),
            description: "Search wiki by keyword and return paragraph frames".to_string(),
            input_schema: input_keyword.clone(),
            output_schema: output_paragraph_frames.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: Some(ResourceQuota {
                cpu_ms: 100,
                memory_mb: 32,
            }),
            schema_hash: PipeCommandManifest::compute_schema_hash(
                &input_keyword,
                &output_paragraph_frames,
            ),
        },
        PipeCommandManifest {
            command_id: "citation.resolve".to_string(),
            description: "Resolve citations into cited paragraphs".to_string(),
            input_schema: output_paragraph_frames.clone(),
            output_schema: input_cited_paragraphs.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: Some(ResourceQuota {
                cpu_ms: 50,
                memory_mb: 16,
            }),
            schema_hash: PipeCommandManifest::compute_schema_hash(
                &output_paragraph_frames,
                &input_cited_paragraphs,
            ),
        },
        PipeCommandManifest {
            command_id: "adr.summarize".to_string(),
            description: "Summarize cited paragraphs into a text answer".to_string(),
            input_schema: input_cited_paragraphs.clone(),
            output_schema: output_text_answer.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: Some(ResourceQuota {
                cpu_ms: 200,
                memory_mb: 64,
            }),
            schema_hash: PipeCommandManifest::compute_schema_hash(
                &input_cited_paragraphs,
                &output_text_answer,
            ),
        },
        PipeCommandManifest {
            command_id: "filter".to_string(),
            description: "Filter frames by predicate".to_string(),
            input_schema: passthrough_array.clone(),
            output_schema: passthrough_array.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: Some(ResourceQuota {
                cpu_ms: 20,
                memory_mb: 8,
            }),
            schema_hash: PipeCommandManifest::compute_schema_hash(
                &passthrough_array,
                &passthrough_array,
            ),
        },
        PipeCommandManifest {
            command_id: "map".to_string(),
            description: "Map over frames".to_string(),
            input_schema: passthrough_array.clone(),
            output_schema: passthrough_array.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: Some(ResourceQuota {
                cpu_ms: 20,
                memory_mb: 8,
            }),
            schema_hash: PipeCommandManifest::compute_schema_hash(
                &passthrough_array,
                &passthrough_array,
            ),
        },
        PipeCommandManifest {
            command_id: "wiki.patch.propose".to_string(),
            description: "Propose a patch from cited paragraphs".to_string(),
            input_schema: input_cited_paragraphs.clone(),
            output_schema: output_patch_proposal.clone(),
            side_effect_level: SideEffectLevel::ProposalOnly,
            resource_quota: Some(ResourceQuota {
                cpu_ms: 300,
                memory_mb: 128,
            }),
            schema_hash: PipeCommandManifest::compute_schema_hash(
                &input_cited_paragraphs,
                &output_patch_proposal,
            ),
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideEffectFilter {
    All,
    Level(SideEffectLevel),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeCommandSummary {
    pub command_id: String,
    pub description: String,
    pub side_effect_level: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_contains_expected_commands() {
        let registry = CommandRegistry::builtin();
        let commands = registry.list();
        let ids: Vec<_> = commands.iter().map(|c| c.command_id.as_str()).collect();
        assert!(ids.contains(&"wiki.search"));
        assert!(ids.contains(&"citation.resolve"));
        assert!(ids.contains(&"adr.summarize"));
        assert!(ids.contains(&"filter"));
        assert!(ids.contains(&"map"));
        assert!(ids.contains(&"wiki.patch.propose"));
        assert_eq!(ids.len(), 6);
    }

    #[test]
    fn list_by_side_effect_filters_correctly() {
        let registry = CommandRegistry::builtin();
        let none = registry.list_by_side_effect(SideEffectLevel::None);
        assert_eq!(none.len(), 5);

        let proposal = registry.list_by_side_effect(SideEffectLevel::ProposalOnly);
        assert_eq!(proposal.len(), 1);
        assert_eq!(proposal[0].command_id, "wiki.patch.propose");

        let side_effect = registry.list_by_side_effect(SideEffectLevel::SideEffect);
        assert!(side_effect.is_empty());
    }

    #[test]
    fn inspect_returns_full_manifest() {
        let registry = CommandRegistry::builtin();
        let manifest = registry.inspect("wiki.search").expect("wiki.search exists");
        assert_eq!(manifest.command_id, "wiki.search");
        assert!(!manifest.description.is_empty());
        assert!(manifest.schema_hash.len() == 64);
        assert!(manifest.validate_schema_hash());
    }

    #[test]
    fn inspect_unknown_returns_command_not_found() {
        let registry = CommandRegistry::builtin();
        let result = registry.inspect("unknown.command");
        assert!(matches!(result, Err(CommandNotFound(ref id)) if id == "unknown.command"));
    }

    #[test]
    fn register_rejects_schema_hash_mismatch() {
        let mut registry = CommandRegistry::new();
        let manifest = PipeCommandManifest {
            command_id: "test.cmd".to_string(),
            description: "test".to_string(),
            input_schema: serde_json::json!({"type": "string"}),
            output_schema: serde_json::json!({"type": "number"}),
            side_effect_level: SideEffectLevel::None,
            resource_quota: None,
            schema_hash: "deadbeef".to_string(),
        };
        let result = registry.register(manifest);
        assert!(
            matches!(result, Err(RegistrationError::SchemaHashMismatch { .. })),
            "expected schema hash mismatch, got {:?}",
            result
        );
    }

    #[test]
    fn register_accepts_valid_manifest() {
        let mut registry = CommandRegistry::new();
        let input = serde_json::json!({"type": "string"});
        let output = serde_json::json!({"type": "number"});
        let manifest = PipeCommandManifest {
            command_id: "test.cmd".to_string(),
            description: "test".to_string(),
            input_schema: input.clone(),
            output_schema: output.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: None,
            schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
        };
        assert!(registry.register(manifest).is_ok());
        assert!(registry.inspect("test.cmd").is_ok());
    }

    #[test]
    fn register_rejects_duplicate_command_id() {
        let mut registry = CommandRegistry::new();
        let input = serde_json::json!({"type": "string"});
        let output = serde_json::json!({"type": "number"});
        let manifest = PipeCommandManifest {
            command_id: "test.cmd".to_string(),
            description: "test".to_string(),
            input_schema: input.clone(),
            output_schema: output.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: None,
            schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
        };
        registry.register(manifest.clone()).unwrap();
        let result = registry.register(manifest);
        assert!(
            matches!(result, Err(RegistrationError::DuplicateCommandId(ref id)) if id == "test.cmd")
        );
    }

    #[test]
    fn register_rejects_invalid_command_id() {
        let mut registry = CommandRegistry::new();
        let input = serde_json::json!({"type": "string"});
        let output = serde_json::json!({"type": "number"});
        let manifest = PipeCommandManifest {
            command_id: "".to_string(),
            description: "test".to_string(),
            input_schema: input.clone(),
            output_schema: output.clone(),
            side_effect_level: SideEffectLevel::None,
            resource_quota: None,
            schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
        };
        let result = registry.register(manifest);
        assert!(
            matches!(result, Err(RegistrationError::InvalidCommandId(ref id)) if id.is_empty())
        );
    }
}
