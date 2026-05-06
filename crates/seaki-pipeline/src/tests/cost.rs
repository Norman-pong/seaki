use crate::{
    compile, CostEstimator, IntentParser,
};
use crate::intent::MockIntentParser;
use crate::cost::CostConfidence;
use seaki_pipe::registry::CommandRegistry;

fn setup_registry() -> CommandRegistry {
    CommandRegistry::builtin()
}

#[test]
fn cost_estimate_search_summarize() {
    let registry = setup_registry();
    let parser = MockIntentParser::new();
    let graph = parser.parse("search and summarize").unwrap();

    let compiled = compile(&graph, &registry).unwrap();
    let estimator = CostEstimator::new();
    let estimate = estimator.estimate(&compiled);

    assert_eq!(estimate.graph_id, "search_and_summarize");
    assert!(estimate.estimated_cpu_ms > 0);
    assert!(estimate.estimated_memory_mb > 0);
    assert!(estimate.estimated_tokens > 0); // adr.summarize has a token multiplier
    assert_eq!(estimate.confidence, CostConfidence::Medium);
}

#[test]
fn cost_estimate_search_only() {
    let registry = setup_registry();
    let parser = MockIntentParser::new();
    let graph = parser.parse("search").unwrap();

    let compiled = compile(&graph, &registry).unwrap();
    let estimator = CostEstimator::new();
    let estimate = estimator.estimate(&compiled);

    assert_eq!(estimate.estimated_tokens, 0); // wiki.search has no LLM multiplier
    assert_eq!(estimate.confidence, CostConfidence::High);
}
