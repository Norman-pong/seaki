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
        expected: &OutboxStatus,
        next: &OutboxStatus,
    ) -> Result<(), &'static str> {
        let key = {
            let mut items = self.items.lock().unwrap();
            let item = items.get_mut(item_id).ok_or("item not found")?;
            if item.status != *expected {
                return Err("status mismatch");
            }
            item.status = next.clone();
            if *next == OutboxStatus::Sent {
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
