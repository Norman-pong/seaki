use crate::dispatch::*;
use crate::llm::MessageRole;
use crate::session::{Session, SessionClaim, SessionMessage};
use crate::skill::*;
use seaki_pipe::registry::CommandRegistry;
use seaki_policy::CapabilityStore;

fn test_skill_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::new();
    registry
        .register(SkillManifest {
            skill_id: "wiki.search.summarize".to_string(),
            name: "Wiki Search & Summarize".to_string(),
            description: "Search wiki and summarize results".to_string(),
            trigger_patterns: vec![
                "search".to_string(),
                "find".to_string(),
                "look up".to_string(),
            ],
            required_capabilities: vec![],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: PipelineTemplate {
                steps: vec![
                    TemplateStep {
                        step_id: "s1".to_string(),
                        command_id: "wiki.search".to_string(),
                        args_template: serde_json::json!({"keyword": "{{intent}}"}),
                        input_binding: "constant".to_string(),
                    },
                    TemplateStep {
                        step_id: "s2".to_string(),
                        command_id: "adr.summarize".to_string(),
                        args_template: serde_json::json!({}),
                        input_binding: "previous".to_string(),
                    },
                ],
            },
            priority: 1,
            requires_confirmation: false,
        })
        .unwrap();
    registry
}

fn empty_session() -> Session {
    Session {
        session_id: "sess-001".to_string(),
        workspace_id: "ws-001".to_string(),
        actor_id: "actor-001".to_string(),
        messages: vec![],
        claims: vec![],
        created_at_ms: 0,
        updated_at_ms: 0,
    }
}

fn session_with_claims(claims: Vec<SessionClaim>) -> Session {
    Session {
        session_id: "sess-001".to_string(),
        workspace_id: "ws-001".to_string(),
        actor_id: "actor-001".to_string(),
        messages: vec![],
        claims,
        created_at_ms: 0,
        updated_at_ms: 0,
    }
}

fn session_with_messages(messages: Vec<SessionMessage>) -> Session {
    Session {
        session_id: "sess-001".to_string(),
        workspace_id: "ws-001".to_string(),
        actor_id: "actor-001".to_string(),
        messages,
        claims: vec![],
        created_at_ms: 0,
        updated_at_ms: 0,
    }
}

#[test]
fn dispatch_exact_match_produces_pipeline() {
    let registry = test_skill_registry();
    let dispatcher = SkillDispatcher::new(registry);
    let session = empty_session();
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher.dispatch(
        "search for Rust",
        &session,
        &capability_store,
        &command_registry,
    );
    assert!(result.is_ok());

    let dispatch_result = result.unwrap();
    assert_eq!(dispatch_result.skill_id, "wiki.search.summarize");
    assert_eq!(dispatch_result.pipeline.steps.len(), 2);
    assert_eq!(dispatch_result.pipeline.steps[0].step_id, "s1");
    assert_eq!(dispatch_result.pipeline.steps[1].step_id, "s2");
}

#[test]
fn dispatch_no_match_returns_error() {
    let registry = test_skill_registry();
    let dispatcher = SkillDispatcher::new(registry);
    let session = empty_session();
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher.dispatch("foobar", &session, &capability_store, &command_registry);
    assert_eq!(result, Err(DispatchError::NoMatchingSkill));
}

#[test]
fn dispatch_missing_capability_rejected() {
    let mut registry = test_skill_registry();
    let mut skill = registry.get("wiki.search.summarize").unwrap().clone();
    skill.skill_id = "wiki.search.summarize.cap".to_string();
    skill.required_capabilities = vec!["file.read".to_string()];
    skill.priority = 0; // Higher priority so it sorts first.
    registry.register(skill).unwrap();

    let dispatcher = SkillDispatcher::new(registry);
    let session = empty_session();
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher.dispatch("search", &session, &capability_store, &command_registry);
    assert!(matches!(
        result,
        Err(DispatchError::SkillNotAllowed {
            skill_id,
            reason,
        }) if skill_id == "wiki.search.summarize.cap" && reason.contains("missing capabilities")
    ));
}

#[test]
fn dispatch_pipeline_renders_intent_variable() {
    let registry = test_skill_registry();
    let dispatcher = SkillDispatcher::new(registry);
    let session = empty_session();
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher
        .dispatch(
            "find memory safety",
            &session,
            &capability_store,
            &command_registry,
        )
        .unwrap();

    let step1_args = &result.pipeline.steps[0].args;
    assert_eq!(
        step1_args,
        &serde_json::json!({"keyword": "find memory safety"})
    );
}

#[test]
fn dispatch_pipeline_renders_memory_variable() {
    let mut registry = SkillRegistry::new();
    registry
        .register(SkillManifest {
            skill_id: "memory.echo".to_string(),
            name: "Memory Echo".to_string(),
            description: "Echo memory items".to_string(),
            trigger_patterns: vec!["echo".to_string()],
            required_capabilities: vec![],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: PipelineTemplate {
                steps: vec![TemplateStep {
                    step_id: "s1".to_string(),
                    command_id: "wiki.search".to_string(),
                    args_template: serde_json::json!({"keyword": "{{memory.0}}"}),
                    input_binding: "constant".to_string(),
                }],
            },
            priority: 1,
            requires_confirmation: false,
        })
        .unwrap();

    let dispatcher = SkillDispatcher::new(registry);
    let claims = vec![SessionClaim {
        claim_id: "c1".to_string(),
        text: "rust ownership".to_string(),
        source_seq: 1,
        confidence: 0.9,
    }];
    let session = session_with_claims(claims);
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher
        .dispatch("echo", &session, &capability_store, &command_registry)
        .unwrap();

    let step1_args = &result.pipeline.steps[0].args;
    assert_eq!(
        step1_args,
        &serde_json::json!({"keyword": "rust ownership"})
    );
}

#[test]
fn dispatch_pipeline_command_not_found() {
    let mut registry = SkillRegistry::new();
    registry
        .register(SkillManifest {
            skill_id: "bad.command".to_string(),
            name: "Bad Command".to_string(),
            description: "Uses a non-existent command".to_string(),
            trigger_patterns: vec!["bad".to_string()],
            required_capabilities: vec![],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: PipelineTemplate {
                steps: vec![TemplateStep {
                    step_id: "s1".to_string(),
                    command_id: "nonexistent.command".to_string(),
                    args_template: serde_json::json!({}),
                    input_binding: "constant".to_string(),
                }],
            },
            priority: 1,
            requires_confirmation: false,
        })
        .unwrap();

    let dispatcher = SkillDispatcher::new(registry);
    let session = empty_session();
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::new();

    let result = dispatcher.dispatch("bad", &session, &capability_store, &command_registry);
    assert_eq!(
        result,
        Err(DispatchError::CommandNotFound {
            command_id: "nonexistent.command".to_string(),
        })
    );
}

#[test]
fn dispatch_requires_confirmation_flag() {
    let mut registry = SkillRegistry::new();
    registry
        .register(SkillManifest {
            skill_id: "confirm.me".to_string(),
            name: "Confirm Me".to_string(),
            description: "Requires confirmation".to_string(),
            trigger_patterns: vec!["confirm".to_string()],
            required_capabilities: vec![],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: PipelineTemplate {
                steps: vec![TemplateStep {
                    step_id: "s1".to_string(),
                    command_id: "wiki.search".to_string(),
                    args_template: serde_json::json!({"keyword": "{{intent}}"}),
                    input_binding: "constant".to_string(),
                }],
            },
            priority: 1,
            requires_confirmation: true,
        })
        .unwrap();

    let dispatcher = SkillDispatcher::new(registry);
    let session = empty_session();
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher
        .dispatch("confirm", &session, &capability_store, &command_registry)
        .unwrap();

    assert!(result.requires_confirmation);
}

#[test]
fn dispatch_injected_context_contains_claims() {
    let registry = test_skill_registry();
    let dispatcher = SkillDispatcher::new(registry);

    let claims = vec![
        SessionClaim {
            claim_id: "c1".to_string(),
            text: "first claim".to_string(),
            source_seq: 1,
            confidence: 0.9,
        },
        SessionClaim {
            claim_id: "c2".to_string(),
            text: "second claim".to_string(),
            source_seq: 2,
            confidence: 0.8,
        },
    ];
    let session = session_with_claims(claims);
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher
        .dispatch("search", &session, &capability_store, &command_registry)
        .unwrap();

    assert_eq!(
        result.injected_context.memory_items,
        vec!["first claim", "second claim"]
    );
    assert_eq!(
        result.injected_context.wiki_claims,
        vec!["first claim", "second claim"]
    );
}

#[test]
fn dispatch_injected_context_session_summary_from_user_messages() {
    let registry = test_skill_registry();
    let dispatcher = SkillDispatcher::new(registry);

    let messages = vec![
        SessionMessage {
            seq: 1,
            role: MessageRole::User,
            content: "Hello world".to_string(),
            timestamp_ms: 0,
            metadata: serde_json::Value::Null,
        },
        SessionMessage {
            seq: 2,
            role: MessageRole::Assistant,
            content: "Hi there".to_string(),
            timestamp_ms: 0,
            metadata: serde_json::Value::Null,
        },
        SessionMessage {
            seq: 3,
            role: MessageRole::User,
            content: "How are you".to_string(),
            timestamp_ms: 0,
            metadata: serde_json::Value::Null,
        },
    ];
    let session = session_with_messages(messages);
    let capability_store = CapabilityStore::new();
    let command_registry = CommandRegistry::builtin();

    let result = dispatcher
        .dispatch("search", &session, &capability_store, &command_registry)
        .unwrap();

    assert_eq!(
        result.injected_context.session_summary,
        "Hello world How are you"
    );
}
