use crate::{compile, IntentParser, PolicyEstimator};
use crate::intent::MockIntentParser;
use seaki_pipe::registry::CommandRegistry;
use seaki_pipe::registry::SideEffectLevel;
use std::collections::HashSet;

fn setup_registry() -> CommandRegistry {
    CommandRegistry::builtin()
}

#[test]
fn policy_estimator_flags_approval_for_side_effect() {
    let registry = setup_registry();
    let parser = MockIntentParser::new();
    let graph = parser.parse("propose a patch").unwrap();

    let compiled = compile(&graph, &registry).unwrap();
    let estimator = PolicyEstimator::new();
    let caps = HashSet::from([
        "pipe.command.wiki.search".to_string(),
        "pipe.command.citation.resolve".to_string(),
        "pipe.command.wiki.patch.propose".to_string(),
    ]);
    let estimate = estimator.estimate(&compiled, &caps);

    assert!(estimate.requires_approval);
    assert_eq!(estimate.max_side_effect, SideEffectLevel::ProposalOnly);
    assert!(estimate
        .required_capabilities
        .contains(&"pipe.command.wiki.patch.propose".to_string()));
    assert!(estimate.missing_capabilities.is_empty());
}

#[test]
fn policy_estimator_no_approval_for_readonly_when_actor_has_all_caps() {
    let registry = setup_registry();
    let parser = MockIntentParser::new();
    let graph = parser.parse("search wiki").unwrap();

    let compiled = compile(&graph, &registry).unwrap();
    let estimator = PolicyEstimator::new();
    let caps = HashSet::from(["pipe.command.wiki.search".to_string()]);
    let estimate = estimator.estimate(&compiled, &caps);

    assert!(!estimate.requires_approval);
    assert_eq!(estimate.max_side_effect, SideEffectLevel::None);
    assert!(estimate.missing_capabilities.is_empty());
}

#[test]
fn policy_estimator_flags_approval_when_actor_missing_capability() {
    let registry = setup_registry();
    let parser = MockIntentParser::new();
    let graph = parser.parse("search wiki").unwrap();

    let compiled = compile(&graph, &registry).unwrap();
    let estimator = PolicyEstimator::new();
    let caps = HashSet::new();
    let estimate = estimator.estimate(&compiled, &caps);

    assert!(estimate.requires_approval);
    assert_eq!(estimate.max_side_effect, SideEffectLevel::None);
    assert_eq!(
        estimate.missing_capabilities,
        vec!["pipe.command.wiki.search".to_string()]
    );
}

#[test]
fn policy_estimator_flags_approval_for_side_effect_even_with_caps() {
    let registry = setup_registry();
    let parser = MockIntentParser::new();
    let graph = parser.parse("propose a patch").unwrap();

    let compiled = compile(&graph, &registry).unwrap();
    let estimator = PolicyEstimator::new();
    let caps = HashSet::from([
        "pipe.command.wiki.search".to_string(),
        "pipe.command.citation.resolve".to_string(),
        "pipe.command.wiki.patch.propose".to_string(),
    ]);
    let estimate = estimator.estimate(&compiled, &caps);

    assert!(estimate.requires_approval);
    assert_eq!(estimate.max_side_effect, SideEffectLevel::ProposalOnly);
    assert!(estimate.missing_capabilities.is_empty());
}
