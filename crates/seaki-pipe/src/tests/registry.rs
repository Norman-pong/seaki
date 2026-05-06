use crate::*;

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
        input_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
        output_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
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
        input_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
        output_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
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
        input_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
        output_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
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
        input_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
        output_frame: (crate::FrameType::JsonValue, crate::Cardinality::One),
        side_effect_level: SideEffectLevel::None,
        resource_quota: None,
        schema_hash: PipeCommandManifest::compute_schema_hash(&input, &output),
    };
    let result = registry.register(manifest);
    assert!(matches!(result, Err(RegistrationError::InvalidCommandId(ref id)) if id.is_empty()));
}
