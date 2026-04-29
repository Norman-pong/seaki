//! Outbox: `ChannelActionGrant`, idempotency, lease, retry.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxStatus {
    Pending,
    Leased,
    Sending,
    Sent,
    Failed,
    Retry,
    Compensated,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxItem {
    pub id: String,
    pub channel_event_id: String,
    pub payload: String,
    pub idempotency_key: String,
    pub status: OutboxStatus,
    pub created_at: SystemTime,
    pub lease_expires_at: Option<SystemTime>,
    pub lease_holder: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSendAttempt {
    pub attempt_id: String,
    pub outbox_item_id: String,
    pub status: OutboxStatus,
    pub provider_response: String,
    pub attempted_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderQueryResult {
    Sent,
    NotFound,
    Failed,
}

/// Fake provider query API used to resolve `Unknown` status.
pub trait FakeProviderQueryAPI {
    fn query(&self, provider_idempotency_key: &str) -> ProviderQueryResult;
}

pub struct Outbox {
    items: Mutex<HashMap<String, OutboxItem>>,
    attempts: Mutex<Vec<ChannelSendAttempt>>,
    sent_idempotency_keys: Mutex<HashMap<String, bool>>,
}

impl Outbox {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
            attempts: Mutex::new(Vec::new()),
            sent_idempotency_keys: Mutex::new(HashMap::new()),
        }
    }

    /// Enqueue an item.  Rejects duplicate `id` or already-sent `idempotency_key`.
    pub fn enqueue(&self, item: OutboxItem) -> Result<(), &'static str> {
        let sent = self.sent_idempotency_keys.lock().unwrap();
        if sent.contains_key(&item.idempotency_key) {
            return Err("idempotency key already sent");
        }
        drop(sent);
        let mut items = self.items.lock().unwrap();
        if items.contains_key(&item.id) {
            return Err("duplicate item id");
        }
        items.insert(item.id.clone(), item);
        Ok(())
    }

    /// Try to lease a `Pending` or `Retry` item.  Only one worker succeeds.
    pub fn lease(
        &self,
        item_id: &str,
        worker_id: &str,
        lease_duration: Duration,
        now: SystemTime,
    ) -> bool {
        let mut items = self.items.lock().unwrap();
        let Some(item) = items.get_mut(item_id) else {
            return false;
        };

        if !matches!(item.status, OutboxStatus::Pending | OutboxStatus::Retry) {
            return false;
        }

        if let Some(expires) = item.lease_expires_at {
            if now < expires {
                return false;
            }
        }

        item.status = OutboxStatus::Leased;
        item.lease_expires_at = Some(now + lease_duration);
        item.lease_holder = Some(worker_id.to_string());
        true
    }

    /// Transition item status after validating current state.
    pub fn transition(
        &self,
        item_id: &str,
        expected: OutboxStatus,
        next: OutboxStatus,
    ) -> Result<(), &'static str> {
        let key = {
            let mut items = self.items.lock().unwrap();
            let item = items.get_mut(item_id).ok_or("item not found")?;
            if item.status != expected {
                return Err("status mismatch");
            }
            item.status = next.clone();
            if next == OutboxStatus::Sent {
                Some(item.idempotency_key.clone())
            } else {
                None
            }
        };
        if let Some(key) = key {
            let mut sent = self.sent_idempotency_keys.lock().unwrap();
            sent.insert(key, true);
        }
        Ok(())
    }

    /// Record a send attempt.
    pub fn record_attempt(&self, attempt: ChannelSendAttempt) -> Result<(), &'static str> {
        let mut attempts = self.attempts.lock().unwrap();
        attempts.push(attempt);
        Ok(())
    }

    /// Resolve an `Unknown` item by querying the provider API.
    /// The returned status is applied to the item.
    pub fn resolve_unknown(
        &self,
        item_id: &str,
        query_api: &dyn FakeProviderQueryAPI,
    ) -> Result<OutboxStatus, &'static str> {
        let (key, new_status) = {
            let mut items = self.items.lock().unwrap();
            let item = items.get_mut(item_id).ok_or("item not found")?;
            if item.status != OutboxStatus::Unknown {
                return Err("item not in unknown state");
            }
            let result = query_api.query(&item.idempotency_key);
            let new_status = match result {
                ProviderQueryResult::Sent => OutboxStatus::Sent,
                ProviderQueryResult::NotFound => OutboxStatus::Retry,
                ProviderQueryResult::Failed => OutboxStatus::Failed,
            };
            item.status = new_status.clone();
            if new_status == OutboxStatus::Sent {
                (Some(item.idempotency_key.clone()), new_status)
            } else {
                (None, new_status)
            }
        };
        if let Some(key) = key {
            let mut sent = self.sent_idempotency_keys.lock().unwrap();
            sent.insert(key, true);
        }
        Ok(new_status)
    }

    pub fn get_item(&self, item_id: &str) -> Option<OutboxItem> {
        let items = self.items.lock().unwrap();
        items.get(item_id).cloned()
    }

    pub fn is_idempotency_key_sent(&self, key: &str) -> bool {
        let sent = self.sent_idempotency_keys.lock().unwrap();
        sent.contains_key(key)
    }
}

impl Default for Outbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

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
            idempotency_key: key.to_string(),
            status: OutboxStatus::Pending,
            created_at: SystemTime::now(),
            lease_expires_at: None,
            lease_holder: None,
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
            .transition("i1", OutboxStatus::Leased, OutboxStatus::Sending)
            .unwrap();
        outbox
            .transition("i1", OutboxStatus::Sending, OutboxStatus::Sent)
            .unwrap();

        assert_eq!(outbox.get_item("i1").unwrap().status, OutboxStatus::Sent);
        assert!(outbox.is_idempotency_key_sent("k1"));
    }

    #[test]
    fn failed_to_compensated() {
        let outbox = Outbox::new();
        let mut i = item("i1", "k1");
        i.status = OutboxStatus::Failed;
        outbox.enqueue(i).unwrap();

        outbox
            .transition("i1", OutboxStatus::Failed, OutboxStatus::Compensated)
            .unwrap();
        assert_eq!(
            outbox.get_item("i1").unwrap().status,
            OutboxStatus::Compensated
        );
    }

    #[test]
    fn duplicate_idempotency_key_cannot_enqueue() {
        let outbox = Outbox::new();
        outbox.enqueue(item("i1", "k1")).unwrap();

        outbox
            .transition("i1", OutboxStatus::Pending, OutboxStatus::Sent)
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
        assert_eq!(outbox.get_item("i1").unwrap().status, OutboxStatus::Retry);
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

        let item = outbox.get_item("i1").unwrap();
        assert!(item.lease_holder.is_some());
    }
}
