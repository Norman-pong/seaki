use crate::skill::*;
use seaki_policy::CapabilityStore;

fn sample_skill(skill_id: &str) -> SkillManifest {
    SkillManifest {
        skill_id: skill_id.to_string(),
        name: format!("Skill {skill_id}"),
        description: "A test skill.".to_string(),
        trigger_patterns: vec!["search".to_string(), "find".to_string()],
        required_capabilities: vec!["file.read".to_string()],
        required_memory_scopes: vec![],
        required_source_scopes: vec![],
        pipeline_template: PipelineTemplate {
            steps: vec![TemplateStep {
                step_id: "step1".to_string(),
                command_id: "cmd.search".to_string(),
                args_template: serde_json::json!({"query": "{{intent}}"}),
                input_binding: "previous".to_string(),
            }],
        },
        priority: 10,
        requires_confirmation: false,
    }
}

#[test]
fn skill_registry_register_and_get() {
    let mut registry = SkillRegistry::new();
    let skill = sample_skill("sk-001");
    assert!(registry.register(skill.clone()).is_ok());

    let retrieved = registry.get("sk-001");
    assert_eq!(retrieved, Some(&skill));
}

#[test]
fn skill_registry_duplicate_id_rejected() {
    let mut registry = SkillRegistry::new();
    let skill = sample_skill("sk-001");
    registry.register(skill.clone()).unwrap();

    let result = registry.register(skill);
    assert_eq!(
        result,
        Err(RegistrationError::DuplicateSkillId("sk-001".to_string()))
    );
}

#[test]
fn skill_registry_empty_triggers_rejected() {
    let mut registry = SkillRegistry::new();
    let mut skill = sample_skill("sk-001");
    skill.trigger_patterns = vec![];

    let result = registry.register(skill);
    assert_eq!(
        result,
        Err(RegistrationError::EmptyTriggerPatterns(
            "sk-001".to_string()
        ))
    );
}

#[test]
fn skill_registry_match_intent_exact() {
    let mut registry = SkillRegistry::new();
    let mut skill = sample_skill("sk-001");
    skill.trigger_patterns = vec!["search".to_string()];
    registry.register(skill).unwrap();

    let matches = registry.match_intent("I want to search for something");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].score, 1.0);
    assert_eq!(matches[0].matched_pattern, "search");
}

#[test]
fn skill_registry_match_intent_case_insensitive() {
    let mut registry = SkillRegistry::new();
    let mut skill = sample_skill("sk-001");
    skill.trigger_patterns = vec!["search".to_string()];
    registry.register(skill).unwrap();

    let matches = registry.match_intent("SEARCH");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].score, 1.0);
}

#[test]
fn skill_registry_match_intent_typo() {
    let mut registry = SkillRegistry::new();
    let mut skill = sample_skill("sk-001");
    skill.trigger_patterns = vec!["search".to_string()];
    registry.register(skill).unwrap();

    // "serch" is edit distance 1 from "search"
    let matches = registry.match_intent("serch");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].score, 0.8);
    assert_eq!(matches[0].matched_pattern, "search");
}

#[test]
fn skill_registry_match_intent_no_match() {
    let mut registry = SkillRegistry::new();
    let mut skill = sample_skill("sk-001");
    skill.trigger_patterns = vec!["search".to_string()];
    registry.register(skill).unwrap();

    let matches = registry.match_intent("foobar");
    assert!(matches.is_empty());
}

#[test]
fn skill_registry_list_by_capability() {
    let mut registry = SkillRegistry::new();
    let mut skill_a = sample_skill("sk-a");
    skill_a.required_capabilities = vec!["file.read".to_string()];
    let mut skill_b = sample_skill("sk-b");
    skill_b.required_capabilities = vec!["wiki.write".to_string()];

    registry.register(skill_a).unwrap();
    registry.register(skill_b).unwrap();

    let file_read_skills = registry.list_by_capability("file.read");
    assert_eq!(file_read_skills.len(), 1);
    assert_eq!(file_read_skills[0].skill_id, "sk-a");
}

#[test]
fn skill_admission_all_capabilities_present() {
    let store = CapabilityStore::new();
    store
        .issue_capability_grant(
            "grant1".to_string(),
            "actor1".to_string(),
            "ws1".to_string(),
            "file.read".to_string(),
            "agent".to_string(),
            "execute".to_string(),
            None,
            None,
            1,
            "admin".to_string(),
        )
        .unwrap()
        .unwrap();

    let mut skill = sample_skill("sk-001");
    skill.required_capabilities = vec!["file.read".to_string()];

    let check = SkillAdmission::check(&skill, &store, "actor1", "ws1").unwrap();
    assert_eq!(check.skill_id, "sk-001");
    assert!(check.allowed);
    assert!(check.missing_capabilities.is_empty());
}

#[test]
fn skill_admission_missing_capability() {
    let store = CapabilityStore::new();
    // No grants issued.

    let mut skill = sample_skill("sk-001");
    skill.required_capabilities = vec!["file.read".to_string(), "wiki.write".to_string()];

    let check = SkillAdmission::check(&skill, &store, "actor1", "ws1").unwrap();
    assert!(!check.allowed);
    assert_eq!(check.missing_capabilities, vec!["file.read", "wiki.write"]);
}

#[test]
fn skill_manifest_serialize_roundtrip() {
    let skill = SkillManifest {
        skill_id: "sk-001".to_string(),
        name: "Test Skill".to_string(),
        description: "Does a thing.".to_string(),
        trigger_patterns: vec!["hello".to_string()],
        required_capabilities: vec!["cap.a".to_string()],
        required_memory_scopes: vec!["mem.x".to_string()],
        required_source_scopes: vec!["src.y".to_string()],
        pipeline_template: PipelineTemplate {
            steps: vec![
                TemplateStep {
                    step_id: "s1".to_string(),
                    command_id: "cmd.a".to_string(),
                    args_template: serde_json::json!({"key": "{{intent}}"}),
                    input_binding: "previous".to_string(),
                },
                TemplateStep {
                    step_id: "s2".to_string(),
                    command_id: "cmd.b".to_string(),
                    args_template: serde_json::json!(null),
                    input_binding: "s1".to_string(),
                },
            ],
        },
        priority: 5,
        requires_confirmation: true,
    };

    let json = serde_json::to_string(&skill).unwrap();
    let deserialized: SkillManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(skill, deserialized);
}
