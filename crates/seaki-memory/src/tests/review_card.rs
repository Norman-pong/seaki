use crate::memory_item::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, TrustLevel,
};
use crate::review_card::{CardDifficulty, ReviewCard};
use seaki_index::IndexScope;

#[test]
fn card_difficulty_default_thresholds() {
    assert!((CardDifficulty::Easy.default_threshold() - 0.65).abs() < f64::EPSILON);
    assert!((CardDifficulty::Medium.default_threshold() - 0.72).abs() < f64::EPSILON);
    assert!((CardDifficulty::Hard.default_threshold() - 0.80).abs() < f64::EPSILON);
    assert!((CardDifficulty::Critical.default_threshold() - 0.90).abs() < f64::EPSILON);
}

#[test]
fn card_difficulty_initial_stability() {
    assert!((CardDifficulty::Easy.initial_stability() - 2.0).abs() < f64::EPSILON);
    assert!((CardDifficulty::Medium.initial_stability() - 1.0).abs() < f64::EPSILON);
    assert!((CardDifficulty::Hard.initial_stability() - 0.5).abs() < f64::EPSILON);
    assert!((CardDifficulty::Critical.initial_stability() - 0.3).abs() < f64::EPSILON);
}

fn dummy_item(kind: MemoryKind) -> MemoryItem {
    MemoryItem {
        memory_id: "mem-1".to_string(),
        kind,
        scope: IndexScope::new("ws", "ac"),
        content: "seaki 的插件不能直接读写本地文件".to_string(),
        source_citation: Some("docs/architecture/channel-bridge.md".to_string()),
        proposed_at: 1000,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: None,
        status: MemoryStatus::Active,
        trust_level: TrustLevel::Confirmed,
        confirmed_by: None,
        provenance: MemoryProvenance {
            origin: MemoryOrigin::WikiPatch,
            extraction_method: "test".to_string(),
            session_id: None,
            wiki_patch_hash: None,
        },
    }
}

#[test]
fn card_from_memory_item() {
    let now = 5000u64;
    let item = dummy_item(MemoryKind::SafetyRule);
    let card = ReviewCard::from_memory_item(&item, now);

    assert_eq!(card.card_id, "card_mem-1");
    assert_eq!(card.memory_id, Some("mem-1".to_string()));
    assert_eq!(card.scope, IndexScope::new("ws", "ac"));
    assert_eq!(card.question, "seaki 的插件不能直接读写本地文件");
    assert_eq!(card.answer, "seaki 的插件不能直接读写本地文件");
    assert_eq!(
        card.source,
        Some("docs/architecture/channel-bridge.md".to_string())
    );
    assert_eq!(card.created_at, 1000);
    assert_eq!(card.last_reviewed_at, None);
    assert_eq!(card.difficulty, CardDifficulty::Critical);
    assert_eq!(card.stability_days, 0.3);
    assert_eq!(card.retention_threshold, 0.90);
    assert_eq!(card.review_count, 0);
    assert_eq!(card.next_review_at, now + (0.3 * 86400.0) as u64);
}

#[test]
fn card_from_memory_item_maps_kind_to_difficulty() {
    let now = 0u64;

    let easy = ReviewCard::from_memory_item(&dummy_item(MemoryKind::UserPreference), now);
    assert_eq!(easy.difficulty, CardDifficulty::Easy);

    let medium1 = ReviewCard::from_memory_item(&dummy_item(MemoryKind::ProjectConvention), now);
    assert_eq!(medium1.difficulty, CardDifficulty::Medium);

    let medium2 = ReviewCard::from_memory_item(&dummy_item(MemoryKind::WorkflowPattern), now);
    assert_eq!(medium2.difficulty, CardDifficulty::Medium);

    let hard = ReviewCard::from_memory_item(&dummy_item(MemoryKind::DerivedFact), now);
    assert_eq!(hard.difficulty, CardDifficulty::Hard);
}
