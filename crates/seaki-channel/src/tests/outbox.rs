use super::*;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

struct MockQueryAPI {
    result: ProviderQueryResult,
}

impl FakeProviderQueryAPI for MockQueryAPI {
    fn query(&self, _provider_idempotency_key: &str) -> ProviderQueryResult {
        self.result.clone()
    }
}

fn item(id: &str, key: &str) -> OutboxItem {
    OutboxItem {
        id: id.to_string(),
        channel_event_id: "evt-1".to_string(),
        payload: "{}".to_string(),
        provider_idempotency_key: key.to_string(),
        status: OutboxStatus::Pending,
        created_at: SystemTime::now(),
        lease_expires_at: None,
        lease_holder: None,
        transaction_id: id.to_string(),
        payload_hash: "hash".to_string(),
        scope: "scope".to_string(),
        audience: "audience".to_string(),
        provider_request_id: None,
        compensating_action: None,
        attempt_count: 0,
        next_attempt_at: None,
        last_error_code: None,
    }
}

#[test]
fn pending_to_leased_to_sent() {
    let outbox = Outbox::new();
    outbox.enqueue(item("i1", "k1")).unwrap();

    let now = SystemTime::now();
    assert!(outbox.lease("i1", "w1", Duration::from_secs(30), now));
    assert!(!outbox.lease("i1", "w2", Duration::from_secs(30), now));

    outbox
        .transition("i1", &OutboxStatus::Leased, &OutboxStatus::Sending)
        .unwrap();
    outbox
        .transition("i1", &OutboxStatus::Sending, &OutboxStatus::Sent)
        .unwrap();

    assert_eq!(outbox.item("i1").unwrap().status, OutboxStatus::Sent);
    assert!(outbox.is_idempotency_key_sent("k1"));
}

#[test]
fn failed_to_compensated() {
    let outbox = Outbox::new();
    let mut i = item("i1", "k1");
    i.status = OutboxStatus::Failed;
    outbox.enqueue(i).unwrap();

    outbox
        .transition("i1", &OutboxStatus::Failed, &OutboxStatus::Compensated)
        .unwrap();
    assert_eq!(outbox.item("i1").unwrap().status, OutboxStatus::Compensated);
}

#[test]
fn duplicate_idempotency_key_cannot_enqueue() {
    let outbox = Outbox::new();
    outbox.enqueue(item("i1", "k1")).unwrap();

    outbox
        .transition("i1", &OutboxStatus::Pending, &OutboxStatus::Sent)
        .unwrap();

    let result = outbox.enqueue(item("i3", "k1"));
    assert_eq!(result, Err("idempotency key already sent"));
}

#[test]
fn unknown_must_query_before_retry() {
    let outbox = Outbox::new();
    let mut i = item("i1", "k1");
    i.status = OutboxStatus::Unknown;
    outbox.enqueue(i).unwrap();

    // Cannot lease unknown directly
    assert!(!outbox.lease("i1", "w1", Duration::from_secs(30), SystemTime::now()));

    let api = MockQueryAPI {
        result: ProviderQueryResult::NotFound,
    };
    let resolved = outbox.resolve_unknown("i1", &api).unwrap();
    assert_eq!(resolved, OutboxStatus::Retry);
    assert_eq!(outbox.item("i1").unwrap().status, OutboxStatus::Retry);
}

#[test]
fn unknown_query_sent_becomes_sent() {
    let outbox = Outbox::new();
    let mut i = item("i1", "k1");
    i.status = OutboxStatus::Unknown;
    outbox.enqueue(i).unwrap();

    let api = MockQueryAPI {
        result: ProviderQueryResult::Sent,
    };
    let resolved = outbox.resolve_unknown("i1", &api).unwrap();
    assert_eq!(resolved, OutboxStatus::Sent);
    assert!(outbox.is_idempotency_key_sent("k1"));
}

#[test]
fn concurrent_lease_only_one_wins() {
    let outbox = Arc::new(Outbox::new());
    outbox.enqueue(item("i1", "k1")).unwrap();

    let mut handles = Vec::new();
    for w in 0..10 {
        let o = Arc::clone(&outbox);
        handles.push(thread::spawn(move || {
            o.lease(
                "i1",
                &format!("worker-{w}"),
                Duration::from_secs(30),
                SystemTime::now(),
            )
        }));
    }

    let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let wins = results.iter().filter(|&&r| r).count();
    assert_eq!(wins, 1);

    let item = outbox.item("i1").unwrap();
    assert!(item.lease_holder.is_some());
}
