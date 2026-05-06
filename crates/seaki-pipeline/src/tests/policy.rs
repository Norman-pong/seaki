use crate::{compile, IntentParser, PolicyEstimator};
use crate::intent::MockIntentParser;
use seaki_pipe::registry::CommandRegistry;
use seaki_pipe::registry::SideEffectLevel;

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
    let estimate = estimator.estimate(&compiled);

    assert!(estimate.requires_approval);
    assert_eq!(estimate.max_side_effect, SideEffectLevel::ProposalOnly);
    assert!(estimate
        .required_capabilities
        .contains(&"pipe.command.wiki.patch.propose".to_string()));
}

#[test]
fn policy_estimator_no_approval_for_readonly() {
    let registry = setup_registry();
    let parser = MockIntentParser::new();
    let graph = parser.parse("search wiki").unwrap();

    let compiled = compile(&graph, &registry).unwrap();
    let estimator = PolicyEstimator::new();
    let estimate = estimator.estimate(&compiled);

    assert!(!estimate.requires_approval);
    assert_eq!(estimate.max_side_effect, SideEffectLevel::None);
}
