use crate::card_generator::CardGenerator;
use crate::memory_item::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, TrustLevel,
};
use crate::memory_store::MemoryStore;
use crate::review_card::CardDifficulty;
use seaki_index::IndexScope;

fn dummy_item_with_content(id: &str, content: &str, status: MemoryStatus) -> MemoryItem {
    MemoryItem {
        memory_id: id.to_string(),
        kind: MemoryKind::ProjectConvention,
        scope: IndexScope::new("ws", "ac"),
        content: content.to_string(),
        source_citation: None,
        proposed_at: 1000,
        confirmed_at: None,
        last_verified_at: None,
        expires_at: None,
        status,
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
fn generator_skips_non_active_items() {
    let gen = CardGenerator::new();
    let item = dummy_item_with_content(
        "mem-1",
        "This is a valid content string.",
        MemoryStatus::Proposed,
    );
    assert!(gen.from_memory_item(&item, 5000).is_none());

    let item2 = dummy_item_with_content(
        "mem-2",
        "This is a valid content string.",
        MemoryStatus::Archived,
    );
    assert!(gen.from_memory_item(&item2, 5000).is_none());
}

#[test]
fn generator_skips_too_short_content() {
    let gen = CardGenerator::new();
    let item = dummy_item_with_content("mem-1", "short", MemoryStatus::Active);
    assert!(gen.from_memory_item(&item, 5000).is_none());
}

#[test]
fn generator_skips_too_long_content() {
    let gen = CardGenerator::new();
    let long_content = "a".repeat(513);
    let item = dummy_item_with_content("mem-1", &long_content, MemoryStatus::Active);
    assert!(gen.from_memory_item(&item, 5000).is_none());
}

#[test]
fn generator_from_store_filters() {
    let mut store = MemoryStore::new(10);
    let active = dummy_item_with_content("mem-a", "This is active content.", MemoryStatus::Active);
    let proposed =
        dummy_item_with_content("mem-b", "This is proposed content.", MemoryStatus::Proposed);
    store.propose(active).unwrap();
    store.propose(proposed).unwrap();

    let gen = CardGenerator::new();
    let cards = gen.generate_from_store(&store, 5000);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].memory_id, Some("mem-a".to_string()));
}

#[test]
fn generator_from_wiki_text() {
    let gen = CardGenerator::new();
    let scope = IndexScope::new("ws", "ac");
    let body =
        "First sentence here. Second sentence there.\nAnother paragraph starts. It continues.";
    let cards = gen.from_wiki_text("WikiPage", body, &scope, 1000);

    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].question, "First sentence here");
    assert_eq!(
        cards[0].answer,
        "First sentence here. Second sentence there."
    );
    assert_eq!(cards[0].source, Some("WikiPage".to_string()));
    assert_eq!(cards[0].difficulty, CardDifficulty::Medium);

    assert_eq!(cards[1].question, "Another paragraph starts");
}

#[test]
fn generator_from_session_summary() {
    let gen = CardGenerator::new();
    let scope = IndexScope::new("ws", "ac");
    let summary = "We discussed many things. You must always check permissions.\nRemember to lock the door. Nothing important here.";
    let cards = gen.from_session_summary(summary, &scope, "sess-1", 2000);

    assert_eq!(cards.len(), 2);
    assert!(cards[0].question.to_lowercase().contains("must"));
    assert!(cards[1].question.to_lowercase().contains("remember"));
    assert_eq!(cards[0].source, Some("session:sess-1".to_string()));
}
