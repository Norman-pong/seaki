use crate::review_card::{CardDifficulty, ReviewCard};
use crate::review_queue::{ReviewQueue, ReviewQueueError};
use seaki_index::IndexScope;

fn dummy_card(card_id: &str, next_review_at: u64) -> ReviewCard {
    ReviewCard {
        card_id: card_id.to_string(),
        memory_id: None,
        scope: IndexScope::new("ws", "ac"),
        question: "q".to_string(),
        answer: "a".to_string(),
        source: None,
        created_at: 0,
        last_reviewed_at: None,
        stability_days: 1.0,
        retention_threshold: 0.72,
        next_review_at,
        review_count: 0,
        difficulty: CardDifficulty::Medium,
    }
}

#[test]
fn review_queue_enqueue_and_due() {
    let q = ReviewQueue::new();
    q.enqueue(dummy_card("c1", 100));
    q.enqueue(dummy_card("c2", 200));
    q.enqueue(dummy_card("c3", 50));

    assert_eq!(q.len(), 3);

    let due = q.due_cards(150);
    assert_eq!(due.len(), 2);
    assert_eq!(due[0].card_id, "c3"); // 50 <= 150
    assert_eq!(due[1].card_id, "c1"); // 100 <= 150
}

#[test]
fn review_queue_next_due_sorted() {
    let q = ReviewQueue::new();
    q.enqueue(dummy_card("c1", 500));
    q.enqueue(dummy_card("c2", 100));
    q.enqueue(dummy_card("c3", 300));
    q.enqueue(dummy_card("c4", 200));

    let next = q.next_due(1000, 2);
    assert_eq!(next.len(), 2);
    assert_eq!(next[0].card_id, "c2"); // 100
    assert_eq!(next[1].card_id, "c4"); // 200
}

#[test]
fn review_queue_upcoming_preview() {
    let q = ReviewQueue::new();
    q.enqueue(dummy_card("c1", 3600)); // 1 小时后
    q.enqueue(dummy_card("c2", 7200)); // 2 小时后
    q.enqueue(dummy_card("c3", 10800)); // 3 小时后
    q.enqueue(dummy_card("c4", 0)); // 已到期，不应出现在 upcoming

    let upcoming = q.upcoming(0, 2);
    assert_eq!(upcoming.len(), 2);
    assert_eq!(upcoming[0].card_id, "c1");
    assert_eq!(upcoming[1].card_id, "c2");
}

#[test]
fn review_queue_update_card() {
    let q = ReviewQueue::new();
    q.enqueue(dummy_card("c1", 100));

    let mut updated = dummy_card("c1", 100);
    updated.stability_days = 5.0;
    updated.next_review_at = 500;
    q.update_card(updated).unwrap();

    let due = q.due_cards(500);
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].stability_days, 5.0);
}

#[test]
fn review_queue_update_card_not_found() {
    let q = ReviewQueue::new();
    q.enqueue(dummy_card("c1", 100));

    let err = q.update_card(dummy_card("missing", 200)).unwrap_err();
    assert_eq!(err, ReviewQueueError::CardNotFound("missing".to_string()));
}

#[test]
fn review_queue_remove() {
    let q = ReviewQueue::new();
    q.enqueue(dummy_card("c1", 100));
    q.enqueue(dummy_card("c2", 200));

    let removed = q.remove("c1");
    assert!(removed.is_some());
    assert_eq!(q.len(), 1);

    let missing = q.remove("c1");
    assert!(missing.is_none());
}

#[test]
fn review_queue_concurrent_enqueue_and_due() {
    use std::thread;

    let q = std::sync::Arc::new(ReviewQueue::new());
    let mut handles = vec![];

    for i in 0..100 {
        let q = q.clone();
        handles.push(thread::spawn(move || {
            q.enqueue(dummy_card(&format!("c{i}"), i as u64 * 10));
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(q.len(), 100);
    let due = q.due_cards(500);
    assert!(due.len() >= 51); // 0..=50 are <= 500
}
