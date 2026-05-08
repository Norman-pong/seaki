use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use seaki_agent::{
    AgentContext, AgentExecutionError, AgentRuntime, AgentRuntimeBuilder, MessageRole,
    MockLlmClient, Session, SessionMessage, SessionState, SessionStateMachine, SkillDispatcher,
    SkillManifest, SkillRegistry, TemplateStep,
};
use seaki_pipe::ast::ComposedStep;
use seaki_pipe::registry::CommandRegistry;
use seaki_pipe::run::{
    AdrSummarizeExecutor, CitationResolveExecutor, CommandExecutor, ExecutionContext, SimplePolicy,
    StepPolicy, WikiPatchProposeExecutor, WikiSearchExecutor,
};

use seaki_policy::{CapabilityStore, PolicyDecision};

fn create_test_skill_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::new();
    registry
        .register(SkillManifest {
            skill_id: "wiki.qa".to_string(),
            name: "Wiki QA".to_string(),
            description: "Answer questions from wiki".to_string(),
            trigger_patterns: vec!["search".to_string(), "find".to_string()],
            required_capabilities: vec!["wiki:read".to_string()],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: seaki_agent::PipelineTemplate {
                steps: vec![
                    TemplateStep {
                        step_id: "search".to_string(),
                        command_id: "wiki.search".to_string(),
                        args_template: serde_json::json!({"keyword": "{{intent}}"}),
                        input_binding: "constant".to_string(),
                    },
                    TemplateStep {
                        step_id: "resolve".to_string(),
                        command_id: "citation.resolve".to_string(),
                        args_template: serde_json::json!({}),
                        input_binding: "previous".to_string(),
                    },
                    TemplateStep {
                        step_id: "summarize".to_string(),
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

fn create_patch_skill_registry() -> SkillRegistry {
    let mut registry = SkillRegistry::new();
    registry
        .register(SkillManifest {
            skill_id: "wiki.patch".to_string(),
            name: "Wiki Patch".to_string(),
            description: "Propose wiki patches".to_string(),
            trigger_patterns: vec!["patch".to_string()],
            required_capabilities: vec!["wiki:propose".to_string()],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: seaki_agent::PipelineTemplate {
                steps: vec![TemplateStep {
                    step_id: "propose".to_string(),
                    command_id: "wiki.patch.propose".to_string(),
                    args_template: serde_json::json!({}),
                    input_binding: "constant".to_string(),
                }],
            },
            priority: 1,
            requires_confirmation: false,
        })
        .unwrap();
    registry
}

fn create_test_command_registry() -> CommandRegistry {
    CommandRegistry::builtin()
}

fn create_test_executors() -> HashMap<String, Box<dyn CommandExecutor>> {
    let mut executors: HashMap<String, Box<dyn CommandExecutor>> = HashMap::new();
    executors.insert("wiki.search".to_string(), Box::new(WikiSearchExecutor));
    executors.insert(
        "citation.resolve".to_string(),
        Box::new(CitationResolveExecutor),
    );
    executors.insert("adr.summarize".to_string(), Box::new(AdrSummarizeExecutor));
    executors.insert(
        "wiki.patch.propose".to_string(),
        Box::new(WikiPatchProposeExecutor),
    );
    executors
}

fn create_test_capability_store() -> CapabilityStore {
    let store = CapabilityStore::new();
    store
        .issue_capability_grant(
            "cap-1".to_string(),
            "actor-1".to_string(),
            "ws-1".to_string(),
            "wiki:read".to_string(),
            "agent".to_string(),
            "execute".to_string(),
            None,
            None,
            1,
            "system".to_string(),
        )
        .unwrap()
        .unwrap();
    store
        .issue_capability_grant(
            "cap-2".to_string(),
            "actor-1".to_string(),
            "ws-1".to_string(),
            "wiki:propose".to_string(),
            "agent".to_string(),
            "execute".to_string(),
            None,
            None,
            1,
            "system".to_string(),
        )
        .unwrap()
        .unwrap();
    store
}

fn create_test_session() -> Session {
    Session {
        session_id: "sess-1".to_string(),
        workspace_id: "ws-1".to_string(),
        actor_id: "actor-1".to_string(),
        messages: vec![SessionMessage {
            seq: 1,
            role: MessageRole::User,
            content: "search for something".to_string(),
            timestamp_ms: 0,
            metadata: serde_json::Value::Null,
        }],
        claims: vec![],
        created_at_ms: 0,
        updated_at_ms: 0,
        approval_timeout_ms: 30000,
    }
}

// ---------------------------------------------------------------------------
// Approval gates for testing
// ---------------------------------------------------------------------------

struct ImmediateApproveGate;

impl seaki_pipe::approval_gate::ApprovalGate for ImmediateApproveGate {
    fn request_approval(
        &self,
        _request: seaki_pipe::approval_gate::ApprovalRequestInput,
    ) -> Result<String, seaki_pipe::approval_gate::ApprovalGateError> {
        Ok("auto".to_string())
    }

    fn poll_approval(
        &self,
        _approval_id: &str,
    ) -> Result<seaki_policy::ApprovalStatus, seaki_pipe::approval_gate::ApprovalGateError> {
        Ok(seaki_policy::ApprovalStatus::Approved)
    }

    fn wait_for_approval(
        &self,
        _approval_id: &str,
        _timeout_ms: u64,
    ) -> Result<seaki_policy::ApprovalStatus, seaki_pipe::approval_gate::ApprovalGateError> {
        Ok(seaki_policy::ApprovalStatus::Approved)
    }
}

struct ImmediateDenyGate;

impl seaki_pipe::approval_gate::ApprovalGate for ImmediateDenyGate {
    fn request_approval(
        &self,
        _request: seaki_pipe::approval_gate::ApprovalRequestInput,
    ) -> Result<String, seaki_pipe::approval_gate::ApprovalGateError> {
        Ok("deny".to_string())
    }

    fn poll_approval(
        &self,
        _approval_id: &str,
    ) -> Result<seaki_policy::ApprovalStatus, seaki_pipe::approval_gate::ApprovalGateError> {
        Ok(seaki_policy::ApprovalStatus::Denied)
    }

    fn wait_for_approval(
        &self,
        _approval_id: &str,
        _timeout_ms: u64,
    ) -> Result<seaki_policy::ApprovalStatus, seaki_pipe::approval_gate::ApprovalGateError> {
        Ok(seaki_policy::ApprovalStatus::Denied)
    }
}

struct RequireApprovalThenAllowPolicy {
    count: AtomicUsize,
}

impl StepPolicy for RequireApprovalThenAllowPolicy {
    fn check(&self, _step: &ComposedStep, _ctx: &ExecutionContext) -> PolicyDecision {
        let c = self.count.fetch_add(1, Ordering::SeqCst);
        if c == 0 {
            PolicyDecision::RequireApproval
        } else {
            PolicyDecision::Allow
        }
    }
}

struct RequireApprovalThenDenyPolicy {
    count: AtomicUsize,
}

impl StepPolicy for RequireApprovalThenDenyPolicy {
    fn check(&self, _step: &ComposedStep, _ctx: &ExecutionContext) -> PolicyDecision {
        let c = self.count.fetch_add(1, Ordering::SeqCst);
        if c == 0 {
            PolicyDecision::RequireApproval
        } else {
            PolicyDecision::Deny
        }
    }
}

/// Only requires approval for a specific step; allows everything else.
struct RequireApprovalForStepPolicy {
    target_step_id: String,
    count: AtomicUsize,
}

impl StepPolicy for RequireApprovalForStepPolicy {
    fn check(&self, step: &ComposedStep, _ctx: &ExecutionContext) -> PolicyDecision {
        if step.step_id == self.target_step_id {
            let c = self.count.fetch_add(1, Ordering::SeqCst);
            if c == 0 {
                PolicyDecision::RequireApproval
            } else {
                PolicyDecision::Allow
            }
        } else {
            PolicyDecision::Allow
        }
    }
}

/// Wraps an executor and counts how many times it is invoked.
struct CountingExecutor {
    count: Arc<AtomicUsize>,
    inner: Box<dyn CommandExecutor>,
}

impl CommandExecutor for CountingExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<seaki_pipe::dry_run::FrameEnvelope>,
        ctx: &mut ExecutionContext,
    ) -> Result<Vec<seaki_pipe::dry_run::FrameEnvelope>, seaki_pipe::dry_run::PipelineError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(step, input, ctx)
    }
}

/// Stub executor that simply passes frames through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PassthroughExecutor;

impl CommandExecutor for PassthroughExecutor {
    fn execute(
        &self,
        step: &ComposedStep,
        input: Vec<seaki_pipe::dry_run::FrameEnvelope>,
        _ctx: &mut ExecutionContext,
    ) -> Result<Vec<seaki_pipe::dry_run::FrameEnvelope>, seaki_pipe::dry_run::PipelineError> {
        Ok(input
            .into_iter()
            .map(|mut f| {
                f.step_id = step.step_id.clone();
                f.frame_type = step.output_type.0;
                f
            })
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn execute_intent_no_matching_skill() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = SkillRegistry::new(); // empty
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = SimplePolicy;
    let executors = create_test_executors();

    let result = runtime.execute_intent(
        "unknown intent",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(matches!(result, Err(AgentExecutionError::NoMatchingSkill)));
}

#[test]
fn execute_intent_pipeline_compile_failed() {
    let mut skill_registry = SkillRegistry::new();
    skill_registry
        .register(SkillManifest {
            skill_id: "bad.skill".to_string(),
            name: "Bad Skill".to_string(),
            description: "Uses unknown command".to_string(),
            trigger_patterns: vec!["bad".to_string()],
            required_capabilities: vec![],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: seaki_agent::PipelineTemplate {
                steps: vec![TemplateStep {
                    step_id: "bad".to_string(),
                    command_id: "unknown.command".to_string(),
                    args_template: serde_json::json!({}),
                    input_binding: "constant".to_string(),
                }],
            },
            priority: 1,
            requires_confirmation: false,
        })
        .unwrap();

    let llm = Box::new(MockLlmClient::new());
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = SimplePolicy;
    let executors = create_test_executors();

    let result = runtime.execute_intent(
        "bad command",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(matches!(
        result,
        Err(AgentExecutionError::PipelineCompileFailed(_))
    ));
}

#[test]
fn execute_intent_full_execution_success() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = create_test_skill_registry();
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = SimplePolicy;
    let executors = create_test_executors();

    let result = runtime.execute_intent(
        "search for something",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(result.is_ok(), "expected success, got {:?}", result);
    let response = result.unwrap();
    assert_eq!(response.pipeline_id, "pipeline:search");
    assert!(!response.audit_trail.is_empty());
    assert!(response.dry_run_result.is_some());
}

#[test]
fn execute_intent_approval_required_then_approved() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = create_patch_skill_registry();
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = RequireApprovalThenAllowPolicy {
        count: AtomicUsize::new(0),
    };
    let mut executors = HashMap::new();
    executors.insert(
        "wiki.patch.propose".to_string(),
        Box::new(WikiPatchProposeExecutor) as Box<dyn CommandExecutor>,
    );

    let result = runtime.execute_intent(
        "patch the wiki",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(
        result.is_ok(),
        "expected success after approval, got {:?}",
        result
    );
    let response = result.unwrap();
    assert!(response.approval_required);
}

#[test]
fn execute_intent_approval_denied() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = create_patch_skill_registry();
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateDenyGate;
    let policy = SimplePolicy;
    let mut executors = HashMap::new();
    executors.insert(
        "wiki.patch.propose".to_string(),
        Box::new(WikiPatchProposeExecutor) as Box<dyn CommandExecutor>,
    );

    let result = runtime.execute_intent(
        "patch the wiki",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(matches!(result, Err(AgentExecutionError::ApprovalDenied)));
}

#[test]
fn execute_intent_approval_then_policy_denies() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = create_patch_skill_registry();
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = RequireApprovalThenDenyPolicy {
        count: AtomicUsize::new(0),
    };
    let mut executors = HashMap::new();
    executors.insert(
        "wiki.patch.propose".to_string(),
        Box::new(WikiPatchProposeExecutor) as Box<dyn CommandExecutor>,
    );

    let result = runtime.execute_intent(
        "patch the wiki",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(
        matches!(result, Err(AgentExecutionError::ExecutionFailed(_))),
        "expected execution failed due to policy deny after approval, got {:?}",
        result
    );
}

#[test]
fn execute_intent_session_state_transitions() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = create_test_skill_registry();
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = SimplePolicy;
    let executors = create_test_executors();

    let _result = runtime.execute_intent(
        "search for something",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    let transitions: Vec<(SessionState, SessionState)> =
        sm.history.iter().map(|t| (t.from, t.to)).collect();
    assert_eq!(
        transitions,
        vec![
            (SessionState::Idle, SessionState::Planning),
            (SessionState::Planning, SessionState::Executing),
            (SessionState::Executing, SessionState::Idle),
        ]
    );
}

#[test]
fn builder_creates_runtime() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = create_test_skill_registry();
    let command_registry = create_test_command_registry();

    let runtime = AgentRuntimeBuilder::new()
        .with_llm(llm)
        .with_skill_registry(skill_registry)
        .with_command_registry(command_registry)
        .build()
        .unwrap();

    assert!(!runtime.command_registry.list().is_empty());
}

#[test]
fn agent_response_contains_answer() {
    let llm = Box::new(MockLlmClient::new());
    let skill_registry = create_test_skill_registry();
    let command_registry = create_test_command_registry();
    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = SimplePolicy;
    let executors = create_test_executors();

    let result = runtime.execute_intent(
        "search for something",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    let response = result.unwrap();
    assert!(!response.answer.is_empty());
}

#[test]
fn execute_intent_approval_retry_preserves_audit() {
    let llm = Box::new(MockLlmClient::new());
    let mut skill_registry = SkillRegistry::new();
    skill_registry
        .register(SkillManifest {
            skill_id: "mixed.pipeline".to_string(),
            name: "Mixed Pipeline".to_string(),
            description: "First step allowed, second needs approval".to_string(),
            trigger_patterns: vec!["mixed".to_string()],
            required_capabilities: vec![],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: seaki_agent::PipelineTemplate {
                steps: vec![
                    TemplateStep {
                        step_id: "filter".to_string(),
                        command_id: "filter".to_string(),
                        args_template: serde_json::json!({"predicate": {}}),
                        input_binding: "constant".to_string(),
                    },
                    TemplateStep {
                        step_id: "write".to_string(),
                        command_id: "external.write".to_string(),
                        args_template: serde_json::json!({}),
                        input_binding: "previous".to_string(),
                    },
                ],
            },
            priority: 1,
            requires_confirmation: false,
        })
        .unwrap();

    let mut command_registry = CommandRegistry::new();
    let passthrough = serde_json::json!({"type": "array"});
    command_registry
        .register(seaki_pipe::registry::PipeCommandManifest {
            command_id: "filter".to_string(),
            description: "filter".to_string(),
            input_schema: passthrough.clone(),
            output_schema: passthrough.clone(),
            input_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            output_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            side_effect_level: seaki_pipe::registry::SideEffectLevel::None,
            resource_quota: None,
            schema_hash: seaki_pipe::registry::PipeCommandManifest::compute_schema_hash(
                &passthrough,
                &passthrough,
            ),
        })
        .unwrap();
    command_registry
        .register(seaki_pipe::registry::PipeCommandManifest {
            command_id: "external.write".to_string(),
            description: "external write".to_string(),
            input_schema: passthrough.clone(),
            output_schema: passthrough.clone(),
            input_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            output_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            side_effect_level: seaki_pipe::registry::SideEffectLevel::ExternalIrreversible,
            resource_quota: None,
            schema_hash: seaki_pipe::registry::PipeCommandManifest::compute_schema_hash(
                &passthrough,
                &passthrough,
            ),
        })
        .unwrap();

    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = RequireApprovalForStepPolicy {
        target_step_id: "write".to_string(),
        count: AtomicUsize::new(0),
    };
    let mut executors = HashMap::new();
    executors.insert(
        "filter".to_string(),
        Box::new(seaki_pipe::run::FilterExecutor) as Box<dyn CommandExecutor>,
    );
    executors.insert(
        "external.write".to_string(),
        Box::new(PassthroughExecutor) as Box<dyn CommandExecutor>,
    );

    let result = runtime.execute_intent(
        "mixed pipeline",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(
        result.is_ok(),
        "expected success after approval, got {:?}",
        result
    );
    let response = result.unwrap();
    assert!(response.approval_required);
    let audit_step_ids: Vec<_> = response
        .audit_trail
        .iter()
        .map(|a| a.step_id.as_str())
        .collect();
    assert!(
        audit_step_ids.contains(&"filter"),
        "audit should contain filter step"
    );
    assert!(
        audit_step_ids.contains(&"write"),
        "audit should contain write step"
    );
}

#[test]
fn execute_intent_approval_retry_does_not_repeat_steps() {
    let llm = Box::new(MockLlmClient::new());
    let mut skill_registry = SkillRegistry::new();
    skill_registry
        .register(SkillManifest {
            skill_id: "mixed.pipeline".to_string(),
            name: "Mixed Pipeline".to_string(),
            description: "First step allowed, second needs approval".to_string(),
            trigger_patterns: vec!["mixed".to_string()],
            required_capabilities: vec![],
            required_memory_scopes: vec![],
            required_source_scopes: vec![],
            pipeline_template: seaki_agent::PipelineTemplate {
                steps: vec![
                    TemplateStep {
                        step_id: "filter".to_string(),
                        command_id: "filter".to_string(),
                        args_template: serde_json::json!({"predicate": {}}),
                        input_binding: "constant".to_string(),
                    },
                    TemplateStep {
                        step_id: "write".to_string(),
                        command_id: "external.write".to_string(),
                        args_template: serde_json::json!({}),
                        input_binding: "previous".to_string(),
                    },
                ],
            },
            priority: 1,
            requires_confirmation: false,
        })
        .unwrap();

    let mut command_registry = CommandRegistry::new();
    let passthrough = serde_json::json!({"type": "array"});
    command_registry
        .register(seaki_pipe::registry::PipeCommandManifest {
            command_id: "filter".to_string(),
            description: "filter".to_string(),
            input_schema: passthrough.clone(),
            output_schema: passthrough.clone(),
            input_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            output_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            side_effect_level: seaki_pipe::registry::SideEffectLevel::None,
            resource_quota: None,
            schema_hash: seaki_pipe::registry::PipeCommandManifest::compute_schema_hash(
                &passthrough,
                &passthrough,
            ),
        })
        .unwrap();
    command_registry
        .register(seaki_pipe::registry::PipeCommandManifest {
            command_id: "external.write".to_string(),
            description: "external write".to_string(),
            input_schema: passthrough.clone(),
            output_schema: passthrough.clone(),
            input_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            output_frame: (
                seaki_pipe::ast::FrameType::JsonValue,
                seaki_pipe::ast::Cardinality::Many,
            ),
            side_effect_level: seaki_pipe::registry::SideEffectLevel::ExternalIrreversible,
            resource_quota: None,
            schema_hash: seaki_pipe::registry::PipeCommandManifest::compute_schema_hash(
                &passthrough,
                &passthrough,
            ),
        })
        .unwrap();

    let runtime =
        AgentRuntime::with_components(llm, SkillDispatcher::new(skill_registry), command_registry);

    let mut session = create_test_session();
    let mut sm = SessionStateMachine::new(session.session_id.clone());
    let capability_store = create_test_capability_store();
    let gate = ImmediateApproveGate;
    let policy = RequireApprovalForStepPolicy {
        target_step_id: "write".to_string(),
        count: AtomicUsize::new(0),
    };

    let filter_count = Arc::new(AtomicUsize::new(0));
    let mut executors = HashMap::new();
    executors.insert(
        "filter".to_string(),
        Box::new(CountingExecutor {
            count: Arc::clone(&filter_count),
            inner: Box::new(seaki_pipe::run::FilterExecutor),
        }) as Box<dyn CommandExecutor>,
    );
    executors.insert(
        "external.write".to_string(),
        Box::new(PassthroughExecutor) as Box<dyn CommandExecutor>,
    );

    let result = runtime.execute_intent(
        "mixed pipeline",
        &mut session,
        &mut sm,
        &capability_store,
        &gate,
        &policy,
        &executors,
    );

    assert!(result.is_ok(), "expected success, got {:?}", result);
    assert_eq!(
        filter_count.load(Ordering::SeqCst),
        1,
        "filter step should not be re-executed on retry"
    );
}

#[test]
fn agent_error_display() {
    assert_eq!(
        AgentExecutionError::NoMatchingSkill.to_string(),
        "no matching skill found for intent"
    );
    assert_eq!(
        AgentExecutionError::SkillNotAllowed {
            skill_id: "s1".to_string(),
            reason: "missing cap".to_string(),
        }
        .to_string(),
        "skill s1 not allowed: missing cap"
    );
    assert_eq!(
        AgentExecutionError::PipelineCompileFailed("bad step".to_string()).to_string(),
        "pipeline compile failed: bad step"
    );
    assert_eq!(
        AgentExecutionError::DryRunFailed("oops".to_string()).to_string(),
        "dry run failed: oops"
    );
    assert_eq!(
        AgentExecutionError::ApprovalRequired {
            step_id: "s1".to_string(),
            operation: "write".to_string(),
        }
        .to_string(),
        "approval required for step s1: write"
    );
    assert_eq!(
        AgentExecutionError::ApprovalDenied.to_string(),
        "approval denied"
    );
    assert_eq!(
        AgentExecutionError::ExecutionFailed("boom".to_string()).to_string(),
        "execution failed: boom"
    );
}

#[test]
fn propose_pipeline_returns_json() {
    let llm = Box::new(MockLlmClient::with_fixed_response(
        r#"{"steps": []}"#.to_string(),
    ));
    let runtime = AgentRuntime::new(llm);
    let ctx = AgentContext {
        workspace_id: "ws".to_string(),
        actor_id: "actor".to_string(),
        session_id: None,
    };
    let result = runtime.propose_pipeline("test intent", &ctx);
    assert!(result.is_ok());
}
