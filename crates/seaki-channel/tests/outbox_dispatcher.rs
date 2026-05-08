use seaki_channel::{
    DispatchResult, FakeProviderDriver, Outbox, OutboxDispatcher, OutboxItem, OutboxStatus,
    ProviderError, ProviderQueryResult, RetryBackoff,
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

fn item(id: &str, key: &str, status: OutboxStatus) -> OutboxItem {
    OutboxItem {
        id: id.to_string(),
        channel_event_id: "evt-1".to_string(),
        payload: "{}".to_string(),
        provider_idempotency_key: key.to_string(),
        status,
        created_at: SystemTime::now(),
        lease_expires_at: None,
        lease_holder: None,
        transaction_id: format!("tx-{id}"),
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

fn backoff() -> RetryBackoff {
    RetryBackoff {
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(60),
        max_retries: 3,
    }
}

#[test]
fn dispatcher_leases_and_sends_pending() {
    let outbox = Outbox::new();
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Pending))
        .unwrap();

    let provider = FakeProviderDriver::new();
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff(),
        now,
    );

    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Sent(id) if id == "o1")));
    assert_eq!(outbox.item("o1").unwrap().status, OutboxStatus::Sent);
}

#[test]
fn dispatcher_retries_on_provider_failure() {
    let outbox = Outbox::new();
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Pending))
        .unwrap();

    let provider = FakeProviderDriver::new();
    provider.set_default_send(Err(ProviderError::Network("fail".to_string())));
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff(),
        now,
    );

    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Failed(id, _) if id == "o1")));

    let item = outbox.item("o1").unwrap();
    assert_eq!(item.status, OutboxStatus::Retry);
    assert_eq!(item.attempt_count, 1);
    assert!(item.next_attempt_at.is_some());
    assert!(item.last_error_code.is_some());
}

#[test]
fn dispatcher_compensates_after_max_retries() {
    let outbox = Outbox::new();
    let mut it = item("o1", "idem-1", OutboxStatus::Pending);
    it.compensating_action = Some("refund".to_string());
    outbox.enqueue(it).unwrap();

    let provider = FakeProviderDriver::new();
    provider.set_default_send(Err(ProviderError::Rejected("bad".to_string())));
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    let backoff = RetryBackoff {
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(60),
        max_retries: 0,
    };

    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff,
        now,
    );

    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Compensated(id) if id == "o1")));
    assert_eq!(outbox.item("o1").unwrap().status, OutboxStatus::Compensated);
}

#[test]
fn dispatcher_resolves_unknown_to_sent() {
    let outbox = Outbox::new();
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Unknown))
        .unwrap();

    let provider = FakeProviderDriver::new();
    provider.set_query_result("idem-1", ProviderQueryResult::Sent);
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff(),
        now,
    );

    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Sent(id) if id == "o1")));
    assert_eq!(outbox.item("o1").unwrap().status, OutboxStatus::Sent);
}

#[test]
fn dispatcher_resolves_unknown_to_retry() {
    let outbox = Outbox::new();
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Unknown))
        .unwrap();

    let provider = FakeProviderDriver::new();
    provider.set_query_result("idem-1", ProviderQueryResult::NotFound);
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff(),
        now,
    );

    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Sent(id) if id == "o1")));
    assert_eq!(outbox.item("o1").unwrap().status, OutboxStatus::Sent);
}

#[test]
fn dispatcher_skips_non_eligible_items() {
    let outbox = Outbox::new();
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Leased))
        .unwrap();
    outbox
        .enqueue(item("o2", "idem-2", OutboxStatus::Sent))
        .unwrap();
    outbox
        .enqueue(item("o3", "idem-3", OutboxStatus::Failed))
        .unwrap();
    outbox
        .enqueue(item("o4", "idem-4", OutboxStatus::Compensated))
        .unwrap();

    let provider = FakeProviderDriver::new();
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff(),
        now,
    );

    assert_eq!(results.len(), 4);
    assert!(results
        .iter()
        .all(|r| matches!(r, DispatchResult::Skipped(_))));
}

#[test]
fn dispatcher_respects_next_attempt_at() {
    let outbox = Outbox::new();
    let mut it = item("o1", "idem-1", OutboxStatus::Retry);
    it.next_attempt_at = Some(SystemTime::now() + Duration::from_secs(10));
    outbox.enqueue(it).unwrap();

    let provider = FakeProviderDriver::new();
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff(),
        now,
    );

    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Skipped(id) if id == "o1")));
    assert_eq!(outbox.item("o1").unwrap().status, OutboxStatus::Retry);
}

#[test]
fn dispatcher_exponential_backoff() {
    let b = RetryBackoff {
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(60),
        max_retries: 5,
    };
    assert_eq!(b.compute_next(0), Some(Duration::from_secs(1)));
    assert_eq!(b.compute_next(1), Some(Duration::from_secs(2)));
    assert_eq!(b.compute_next(2), Some(Duration::from_secs(4)));
    assert_eq!(b.compute_next(3), Some(Duration::from_secs(8)));
    assert_eq!(b.compute_next(4), Some(Duration::from_secs(16)));
    assert_eq!(b.compute_next(5), None);

    let capped = RetryBackoff {
        base_delay: Duration::from_secs(10),
        max_delay: Duration::from_secs(15),
        max_retries: 5,
    };
    assert_eq!(capped.compute_next(0), Some(Duration::from_secs(10)));
    assert_eq!(capped.compute_next(1), Some(Duration::from_secs(15)));
    assert_eq!(capped.compute_next(2), Some(Duration::from_secs(15)));
}

#[test]
fn dispatcher_lease_expires_reclaimed() {
    let outbox = Outbox::new();
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Pending))
        .unwrap();

    let provider = FakeProviderDriver::new();
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    // Worker A leases the item
    assert!(outbox.lease("o1", "worker-a", Duration::from_secs(30), now));
    assert_eq!(
        outbox.item("o1").unwrap().lease_holder,
        Some("worker-a".to_string())
    );

    // Worker B tries to dispatch before expiry — should skip
    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-b",
        Duration::from_secs(30),
        &backoff(),
        now,
    );
    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Skipped(id) if id == "o1")));

    // After expiry, worker B should succeed
    let later = now + Duration::from_secs(31);
    let results = dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-b",
        Duration::from_secs(30),
        &backoff(),
        later,
    );
    assert!(results
        .iter()
        .any(|r| matches!(r, DispatchResult::Sent(id) if id == "o1")));
    assert_eq!(
        outbox.item("o1").unwrap().lease_holder,
        Some("worker-b".to_string())
    );
}

#[test]
fn dispatcher_concurrent_workers_race() {
    let outbox = Arc::new(Outbox::new());
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Pending))
        .unwrap();

    let provider = Arc::new(FakeProviderDriver::new());
    let dispatcher = Arc::new(OutboxDispatcher);
    let now = SystemTime::now();

    let mut handles = Vec::new();
    for w in 0..8 {
        let o = Arc::clone(&outbox);
        let p = Arc::clone(&provider);
        let d = Arc::clone(&dispatcher);
        handles.push(thread::spawn(move || {
            d.dispatch_round(
                &o,
                &*p,
                &format!("worker-{w}"),
                Duration::from_secs(30),
                &backoff(),
                now,
            )
        }));
    }

    let all_results: Vec<DispatchResult> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();

    let sent_count = all_results
        .iter()
        .filter(|r| matches!(r, DispatchResult::Sent(id) if id == "o1"))
        .count();
    assert_eq!(sent_count, 1);
    assert_eq!(outbox.item("o1").unwrap().status, OutboxStatus::Sent);
}

#[test]
fn dispatcher_records_attempts() {
    let outbox = Outbox::new();
    outbox
        .enqueue(item("o1", "idem-1", OutboxStatus::Pending))
        .unwrap();

    let provider = FakeProviderDriver::new();
    provider.set_default_send(Err(ProviderError::Network("fail".to_string())));
    let dispatcher = OutboxDispatcher;
    let now = SystemTime::now();

    dispatcher.dispatch_round(
        &outbox,
        &provider,
        "worker-1",
        Duration::from_secs(30),
        &backoff(),
        now,
    );

    let attempts = outbox.attempts();
    assert_eq!(attempts.len(), 1);
    let a = &attempts[0];
    assert_eq!(a.outbox_item_id, "o1");
    assert_eq!(a.status, OutboxStatus::Sending);
    assert_eq!(a.lease_owner, "worker-1");
    assert_eq!(a.attempt_count, 0);
}

#[test]
fn outbox_enqueue_rejects_duplicate_idempotency() {
    let outbox = Outbox::new();
    let mut it = item("o1", "idem-1", OutboxStatus::Pending);
    outbox.enqueue(it.clone()).unwrap();

    outbox
        .transition("o1", &OutboxStatus::Pending, &OutboxStatus::Sent)
        .unwrap();

    it.id = "o2".to_string();
    let r = outbox.enqueue(it);
    assert_eq!(r, Err("idempotency key already sent"));
}

#[test]
fn retry_backoff_no_u32_overflow() {
    let b = RetryBackoff {
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(3600),
        max_retries: 100,
    };
    // attempt_count >= 32 used to overflow with 2_u32.pow(attempt_count).
    // With saturating_pow it should return max_delay.
    assert_eq!(b.compute_next(31), Some(Duration::from_secs(3600)));
    assert_eq!(b.compute_next(32), Some(Duration::from_secs(3600)));
    assert_eq!(b.compute_next(100), None);
}
