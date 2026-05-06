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
    pub input_frame: crate::TypedFrame,
    pub output_frame: crate::TypedFrame,
    pub side_effect_level: SideEffectLevel,
    pub resource_quota: Option<ResourceQuota>,
    pub schema_hash: String,
}

impl PipeCommandManifest {
    /// Compute the canonical schema hash from `input_schema` and `output_schema`.
    ///
    /// # Panics
    /// Never panics in practice; the hasher is infallible.
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

    /// Create a registry with all built-in commands pre-registered.
    ///
    /// # Panics
    /// Panics if a built-in command has an invalid manifest (should never happen).
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

    /// Register a new command manifest.
    ///
    /// # Errors
    /// Returns `RegistrationError` if the command ID is invalid or already exists.
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

    /// Look up a command manifest by ID.
    ///
    /// # Errors
    /// Returns `CommandNotFound` if the command ID is not registered.
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

    use crate::{Cardinality, FrameType};

    vec![
        PipeCommandManifest {
            command_id: "wiki.search".to_string(),
            description: "Search wiki by keyword and return paragraph frames".to_string(),
            input_schema: input_keyword.clone(),
            output_schema: output_paragraph_frames.clone(),
            input_frame: (FrameType::JsonValue, Cardinality::One),
            output_frame: (FrameType::ParagraphFrame, Cardinality::Many),
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
            input_frame: (FrameType::ParagraphFrame, Cardinality::Many),
            output_frame: (FrameType::CitedParagraph, Cardinality::Many),
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
            input_frame: (FrameType::CitedParagraph, Cardinality::Many),
            output_frame: (FrameType::TextAnswer, Cardinality::One),
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
            input_frame: (FrameType::JsonValue, Cardinality::Many),
            output_frame: (FrameType::JsonValue, Cardinality::Many),
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
            input_frame: (FrameType::JsonValue, Cardinality::Many),
            output_frame: (FrameType::JsonValue, Cardinality::Many),
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
            input_frame: (FrameType::CitedParagraph, Cardinality::Many),
            output_frame: (FrameType::PatchProposalArtifact, Cardinality::One),
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
