use crate::{
    compile, CostEstimator, IntentParser,
};
use crate::compiler::{CompileResult, CompiledStep};
use crate::cost::{ActualCost, CostConfidence};
use crate::intent::MockIntentParser;
use seaki_pipe::registry::CommandRegistry;
use seaki_pipe::{Cardinality, FrameType, ResourceQuota, SideEffectLevel};
use std::collections::HashMap;

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
    // wiki.search (100+32) + citation.resolve (50+16) + adr.summarize (200+64)
    assert_eq!(estimate.estimated_cpu_ms, 350);
    assert_eq!(estimate.estimated_memory_mb, 112);
    // CitedParagraph Many input 2048 + TextAnswer output 512
    assert_eq!(estimate.estimated_tokens, 2560);
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

    assert_eq!(estimate.estimated_cpu_ms, 100);
    assert_eq!(estimate.estimated_memory_mb, 32);
    assert_eq!(estimate.estimated_tokens, 0); // wiki.search has no LLM multiplier
    assert_eq!(estimate.confidence, CostConfidence::High);
}

#[test]
fn cost_estimate_patch_propose_tokens() {
    let estimator = CostEstimator::new();
    let result = CompileResult {
        graph_id: "patch".to_string(),
        linear_steps: vec![CompiledStep {
            step_id: "step1".to_string(),
            command_id: "wiki.patch.propose".to_string(),
            input_type: (FrameType::CitedParagraph, Cardinality::Many),
            output_type: (FrameType::PatchProposalArtifact, Cardinality::One),
            side_effect_level: SideEffectLevel::ProposalOnly,
            resource_quota: Some(ResourceQuota {
                cpu_ms: 300,
                memory_mb: 128,
            }),
            schema_hash: "dummy".to_string(),
        }],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::PatchProposalArtifact, Cardinality::One),
        max_side_effect: SideEffectLevel::ProposalOnly,
        command_schema_hashes: HashMap::new(),
    };

    let est = estimator.estimate(&result);
    assert_eq!(est.estimated_cpu_ms, 300);
    assert_eq!(est.estimated_memory_mb, 128);
    // CitedParagraph Many input 2048 + PatchProposalArtifact output 4096
    assert_eq!(est.estimated_tokens, 6144);
    assert_eq!(est.confidence, CostConfidence::High);
}

#[test]
fn cost_estimate_tokens_by_input_type_and_cardinality() {
    let estimator = CostEstimator::new();

    let cases: Vec<((FrameType, Cardinality), u64)> = vec![
        ((FrameType::ParagraphFrame, Cardinality::Many), 1024 + 512),
        ((FrameType::CitedParagraph, Cardinality::One), 512 + 512),
        ((FrameType::JsonValue, Cardinality::Many), 256 + 512),
        ((FrameType::TextAnswer, Cardinality::One), 128 + 512),
    ];

    for (input_type, expected_tokens) in cases {
        let result = CompileResult {
            graph_id: "test".to_string(),
            linear_steps: vec![CompiledStep {
                step_id: "step".to_string(),
                command_id: "adr.summarize".to_string(),
                input_type,
                output_type: (FrameType::TextAnswer, Cardinality::One),
                side_effect_level: SideEffectLevel::None,
                resource_quota: None,
                schema_hash: "dummy".to_string(),
            }],
            input_type: (FrameType::JsonValue, Cardinality::One),
            output_type: (FrameType::TextAnswer, Cardinality::One),
            max_side_effect: SideEffectLevel::None,
            command_schema_hashes: HashMap::new(),
        };

        let est = estimator.estimate(&result);
        assert_eq!(
            est.estimated_tokens, expected_tokens,
            "unexpected tokens for input {input_type:?}"
        );
    }
}

#[test]
fn cost_estimate_fallback_without_quota() {
    let estimator = CostEstimator::new();
    let result = CompileResult {
        graph_id: "fallback".to_string(),
        linear_steps: vec![CompiledStep {
            step_id: "step".to_string(),
            command_id: "adr.summarize".to_string(),
            input_type: (FrameType::CitedParagraph, Cardinality::Many),
            output_type: (FrameType::TextAnswer, Cardinality::One),
            side_effect_level: SideEffectLevel::None,
            resource_quota: None,
            schema_hash: "dummy".to_string(),
        }],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::TextAnswer, Cardinality::One),
        max_side_effect: SideEffectLevel::None,
        command_schema_hashes: HashMap::new(),
    };

    let est = estimator.estimate(&result);
    assert_eq!(est.estimated_cpu_ms, 100); // DEFAULT_CPU_MS
    assert_eq!(est.estimated_memory_mb, 64); // DEFAULT_MEMORY_MB
}

#[test]
fn cost_estimate_non_llm_without_quota() {
    let estimator = CostEstimator::new();
    let result = CompileResult {
        graph_id: "non_llm".to_string(),
        linear_steps: vec![CompiledStep {
            step_id: "step".to_string(),
            command_id: "filter".to_string(),
            input_type: (FrameType::JsonValue, Cardinality::Many),
            output_type: (FrameType::JsonValue, Cardinality::Many),
            side_effect_level: SideEffectLevel::None,
            resource_quota: None,
            schema_hash: "dummy".to_string(),
        }],
        input_type: (FrameType::JsonValue, Cardinality::Many),
        output_type: (FrameType::JsonValue, Cardinality::Many),
        max_side_effect: SideEffectLevel::None,
        command_schema_hashes: HashMap::new(),
    };

    let est = estimator.estimate(&result);
    assert_eq!(est.estimated_cpu_ms, 100);
    assert_eq!(est.estimated_memory_mb, 64);
    assert_eq!(est.estimated_tokens, 0);
}

#[test]
fn cost_estimate_mixed_quota_presence() {
    let estimator = CostEstimator::new();
    let result = CompileResult {
        graph_id: "mixed".to_string(),
        linear_steps: vec![
            CompiledStep {
                step_id: "a".to_string(),
                command_id: "wiki.search".to_string(),
                input_type: (FrameType::JsonValue, Cardinality::One),
                output_type: (FrameType::ParagraphFrame, Cardinality::Many),
                side_effect_level: SideEffectLevel::None,
                resource_quota: Some(ResourceQuota {
                    cpu_ms: 100,
                    memory_mb: 32,
                }),
                schema_hash: "dummy".to_string(),
            },
            CompiledStep {
                step_id: "b".to_string(),
                command_id: "adr.summarize".to_string(),
                input_type: (FrameType::CitedParagraph, Cardinality::Many),
                output_type: (FrameType::TextAnswer, Cardinality::One),
                side_effect_level: SideEffectLevel::None,
                resource_quota: None,
                schema_hash: "dummy".to_string(),
            },
        ],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::TextAnswer, Cardinality::One),
        max_side_effect: SideEffectLevel::None,
        command_schema_hashes: HashMap::new(),
    };

    let est = estimator.estimate(&result);
    assert_eq!(est.estimated_cpu_ms, 200); // 100 + 100 fallback
    assert_eq!(est.estimated_memory_mb, 96); // 32 + 64 fallback
    assert_eq!(est.estimated_tokens, 2560); // only adr.summarize
}

#[test]
fn cost_estimate_multi_step_accumulation() {
    let estimator = CostEstimator::new();
    let result = CompileResult {
        graph_id: "multi".to_string(),
        linear_steps: vec![
            CompiledStep {
                step_id: "a".to_string(),
                command_id: "wiki.search".to_string(),
                input_type: (FrameType::JsonValue, Cardinality::One),
                output_type: (FrameType::ParagraphFrame, Cardinality::Many),
                side_effect_level: SideEffectLevel::None,
                resource_quota: Some(ResourceQuota {
                    cpu_ms: 100,
                    memory_mb: 32,
                }),
                schema_hash: "dummy".to_string(),
            },
            CompiledStep {
                step_id: "b".to_string(),
                command_id: "citation.resolve".to_string(),
                input_type: (FrameType::ParagraphFrame, Cardinality::Many),
                output_type: (FrameType::CitedParagraph, Cardinality::Many),
                side_effect_level: SideEffectLevel::None,
                resource_quota: Some(ResourceQuota {
                    cpu_ms: 50,
                    memory_mb: 16,
                }),
                schema_hash: "dummy".to_string(),
            },
            CompiledStep {
                step_id: "c".to_string(),
                command_id: "adr.summarize".to_string(),
                input_type: (FrameType::CitedParagraph, Cardinality::Many),
                output_type: (FrameType::TextAnswer, Cardinality::One),
                side_effect_level: SideEffectLevel::None,
                resource_quota: Some(ResourceQuota {
                    cpu_ms: 200,
                    memory_mb: 64,
                }),
                schema_hash: "dummy".to_string(),
            },
        ],
        input_type: (FrameType::JsonValue, Cardinality::One),
        output_type: (FrameType::TextAnswer, Cardinality::One),
        max_side_effect: SideEffectLevel::None,
        command_schema_hashes: HashMap::new(),
    };

    let est = estimator.estimate(&result);
    assert_eq!(est.estimated_cpu_ms, 350);
    assert_eq!(est.estimated_memory_mb, 112);
    assert_eq!(est.estimated_tokens, 2560);
}

#[test]
fn cost_estimate_check_error_within_band() {
    let estimate = crate::cost::CostEstimate {
        graph_id: "test".to_string(),
        estimated_cpu_ms: 100,
        estimated_memory_mb: 64,
        estimated_tokens: 2560,
        confidence: CostConfidence::High,
    };
    let actual = ActualCost {
        cpu_ms: 150,
        memory_mb: 80,
        tokens: 2000,
    };
    assert!(estimate.check_error(&actual).is_ok());
}

#[test]
fn cost_estimate_check_error_lower_bound() {
    let estimate = crate::cost::CostEstimate {
        graph_id: "test".to_string(),
        estimated_cpu_ms: 100,
        estimated_memory_mb: 64,
        estimated_tokens: 1000,
        confidence: CostConfidence::High,
    };
    // ratio = 100 / 199 = 0.5025 -> ok
    let actual_ok = ActualCost {
        cpu_ms: 199,
        memory_mb: 64,
        tokens: 1000,
    };
    assert!(estimate.check_error(&actual_ok).is_ok());
    // ratio = 100 / 201 = 0.4975 -> err
    let actual_err = ActualCost {
        cpu_ms: 201,
        memory_mb: 64,
        tokens: 1000,
    };
    assert!(estimate.check_error(&actual_err).is_err());
}

#[test]
fn cost_estimate_check_error_upper_bound() {
    let estimate = crate::cost::CostEstimate {
        graph_id: "test".to_string(),
        estimated_cpu_ms: 100,
        estimated_memory_mb: 64,
        estimated_tokens: 1000,
        confidence: CostConfidence::High,
    };
    // ratio = 100 / 50 = 2.0 -> ok
    let actual_ok = ActualCost {
        cpu_ms: 50,
        memory_mb: 64,
        tokens: 1000,
    };
    assert!(estimate.check_error(&actual_ok).is_ok());
    // ratio = 100 / 49 = 2.04 -> err
    let actual_err = ActualCost {
        cpu_ms: 49,
        memory_mb: 64,
        tokens: 1000,
    };
    assert!(estimate.check_error(&actual_err).is_err());
}

#[test]
fn cost_estimate_check_error_cpu_outside_band() {
    let estimate = crate::cost::CostEstimate {
        graph_id: "test".to_string(),
        estimated_cpu_ms: 100,
        estimated_memory_mb: 64,
        estimated_tokens: 2560,
        confidence: CostConfidence::High,
    };
    let actual = ActualCost {
        cpu_ms: 400,
        memory_mb: 64,
        tokens: 2560,
    };
    assert!(estimate.check_error(&actual).is_err());
}

#[test]
fn cost_estimate_check_error_memory_outside_band() {
    let estimate = crate::cost::CostEstimate {
        graph_id: "test".to_string(),
        estimated_cpu_ms: 100,
        estimated_memory_mb: 64,
        estimated_tokens: 2560,
        confidence: CostConfidence::High,
    };
    let actual = ActualCost {
        cpu_ms: 100,
        memory_mb: 16,
        tokens: 2560,
    };
    assert!(estimate.check_error(&actual).is_err());
}

#[test]
fn cost_estimate_check_error_tokens_outside_band() {
    let estimate = crate::cost::CostEstimate {
        graph_id: "test".to_string(),
        estimated_cpu_ms: 100,
        estimated_memory_mb: 64,
        estimated_tokens: 2560,
        confidence: CostConfidence::High,
    };
    let actual = ActualCost {
        cpu_ms: 100,
        memory_mb: 64,
        tokens: 30_000,
    };
    assert!(estimate.check_error(&actual).is_err());
}
