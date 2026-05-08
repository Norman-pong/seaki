use crate::memory_item::{
    MemoryItem, MemoryKind, MemoryOrigin, MemoryProvenance, MemoryStatus, TrustLevel,
};
use crate::review_card::ReviewCard;
use crate::topic_clustering::TopicClusterer;
use seaki_index::IndexScope;

fn make_item(id: &str, content: &str) -> MemoryItem {
    MemoryItem {
        memory_id: id.to_string(),
        kind: MemoryKind::DerivedFact,
        scope: IndexScope::new("ws", "ac"),
        content: content.to_string(),
        source_citation: None,
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

fn make_card(id: &str, question: &str, answer: &str) -> ReviewCard {
    ReviewCard {
        card_id: id.to_string(),
        memory_id: None,
        scope: IndexScope::new("ws", "ac"),
        question: question.to_string(),
        answer: answer.to_string(),
        source: None,
        created_at: 1000,
        last_reviewed_at: None,
        stability_days: 1.0,
        retention_threshold: 0.72,
        next_review_at: 2000,
        review_count: 0,
        difficulty: crate::review_card::CardDifficulty::Medium,
    }
}

#[test]
fn clusterer_groups_similar_items() {
    let clusterer = TopicClusterer::new();
    let items = [
        make_item("mem-1", "authentication oauth token security"),
        make_item("mem-2", "authentication token oauth security"),
    ];
    let refs: Vec<&MemoryItem> = items.iter().collect();
    let topics = clusterer.cluster_memory_items(&refs, 0.3, 5000);

    assert_eq!(topics.len(), 1);
    assert_eq!(topics[0].memory_ids.len(), 2);
}

#[test]
fn clusterer_separates_dissimilar_items() {
    let clusterer = TopicClusterer::new();
    let items = [
        make_item("mem-1", "authentication oauth token security"),
        make_item("mem-2", "kubernetes deployment container orchestration"),
    ];
    let refs: Vec<&MemoryItem> = items.iter().collect();
    let topics = clusterer.cluster_memory_items(&refs, 0.3, 5000);

    assert_eq!(topics.len(), 2);
}

#[test]
fn clusterer_assigns_new_item() {
    let clusterer = TopicClusterer::new();
    let items = [
        make_item("mem-1", "authentication oauth token security"),
        make_item("mem-2", "authentication token oauth security"),
    ];
    let refs: Vec<&MemoryItem> = items.iter().collect();
    let mut topics = clusterer.cluster_memory_items(&refs, 0.3, 5000);

    let new_keywords = vec!["oauth".to_string(), "security".to_string()];
    let topic_id = clusterer.assign_to_topic(&new_keywords, &mut topics, 0.3, 6000);

    assert!(topic_id.starts_with("topic_"));
    // 应该分配到已有 topic（因为关键词高度重叠）
    assert_eq!(topic_id, "topic_0");
}

#[test]
fn clusterer_assigns_new_item_creates_topic() {
    let clusterer = TopicClusterer::new();
    let items = [make_item("mem-1", "authentication oauth token security")];
    let refs: Vec<&MemoryItem> = items.iter().collect();
    let mut topics = clusterer.cluster_memory_items(&refs, 0.3, 5000);

    let new_keywords = vec!["kubernetes".to_string(), "deployment".to_string()];
    let topic_id = clusterer.assign_to_topic(&new_keywords, &mut topics, 0.3, 6000);

    assert!(topic_id.starts_with("topic_auto_"));
    assert_eq!(topics.len(), 2);
}

#[test]
fn clusterer_cluster_cards() {
    let clusterer = TopicClusterer::new();
    let cards = [
        make_card("c1", "authentication flow", "oauth and token security"),
        make_card("c2", "secure tokens", "oauth authentication security"),
    ];
    let refs: Vec<&ReviewCard> = cards.iter().collect();
    let topics = clusterer.cluster_cards(&refs, 0.3, 5000);

    assert_eq!(topics.len(), 1);
    assert_eq!(topics[0].card_ids.len(), 2);
}
