use super::*;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime};
use tracing::field::{Field, Visit};
use tracing::{span, Event, Id, Metadata, Subscriber};

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

type WarnEventList = Vec<Vec<(String, String)>>;

#[derive(Debug, Clone, Default)]
struct WarnCapture {
    events: Arc<Mutex<WarnEventList>>,
}

struct WarnVisitor {
    fields: Vec<(String, String)>,
}

impl Visit for WarnVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .push((field.name().to_string(), format!("{value:?}")));
    }
}

impl Subscriber for WarnCapture {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _span: &span::Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _span: &Id, _values: &span::Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut visitor = WarnVisitor { fields: Vec::new() };
        event.record(&mut visitor);
        self.events.lock().unwrap().push(visitor.fields);
    }
    fn enter(&self, _span: &Id) {}
    fn exit(&self, _span: &Id) {}
}

#[test]
fn record_attempt_failure_not_silently_discarded() {
    use crate::outbox::TEST_FORCE_RECORD_ATTEMPT_FAIL;
    use std::sync::atomic::Ordering;

    let capture = WarnCapture::default();
    let events = capture.events.clone();

    TEST_FORCE_RECORD_ATTEMPT_FAIL.store(true, Ordering::Relaxed);

    tracing::subscriber::with_default(capture, || {
        let outbox = Outbox::new();
        outbox.enqueue(item("i1", "k1")).unwrap();

        let dispatcher = OutboxDispatcher;
        let backoff = RetryBackoff {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            max_retries: 3,
        };
        let provider = FakeProviderDriver::new();

        let results = dispatcher.dispatch_round(
            &outbox,
            &provider,
            "worker-1",
            Duration::from_secs(30),
            &backoff,
            SystemTime::now(),
        );

        // The item should still be sent (record_attempt failure is warned, not aborted)
        assert!(results
            .iter()
            .any(|r| matches!(r, DispatchResult::Sent(id) if id == "i1")));
    });

    TEST_FORCE_RECORD_ATTEMPT_FAIL.store(false, Ordering::Relaxed);

    let logs = events.lock().unwrap();
    assert!(
        logs.iter().any(|fields| {
            let map: std::collections::HashMap<String, String> = fields.iter().cloned().collect();
            map.get("message") == Some(&"record_attempt failed".to_string())
                && map.contains_key("error")
                && map.get("error") == Some(&"injected test failure".to_string())
        }),
        "expected a warning log when record_attempt fails, got logs: {:?}",
        *logs
    );
}
