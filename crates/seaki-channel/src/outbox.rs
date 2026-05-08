//! Outbox: `ChannelActionGrant`, idempotency, lease, retry.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use tracing::warn;

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
pub(crate) static TEST_FORCE_RECORD_ATTEMPT_FAIL: AtomicBool = AtomicBool::new(false);

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
    pub provider_idempotency_key: String,
    pub status: OutboxStatus,
    pub created_at: SystemTime,
    pub lease_expires_at: Option<SystemTime>,
    pub lease_holder: Option<String>,
    pub transaction_id: String,
    pub payload_hash: String,
    pub scope: String,
    pub audience: String,
    pub provider_request_id: Option<String>,
    pub compensating_action: Option<String>,
    pub attempt_count: u32,
    pub next_attempt_at: Option<SystemTime>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSendAttempt {
    pub attempt_id: String,
    pub outbox_item_id: String,
    pub status: OutboxStatus,
    pub provider_response: String,
    pub attempted_at: SystemTime,
    pub lease_owner: String,
    pub lease_until: SystemTime,
    pub attempt_count: u32,
    pub next_attempt_at: Option<SystemTime>,
    pub last_error_code: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    Network(String),
    RateLimited,
    Rejected(String),
    Unknown(String),
}

/// Provider driver for sending items and querying idempotency.
pub trait ProviderDriver: Send + Sync {
    fn send(&self, item: &OutboxItem) -> Result<(), ProviderError>;
    fn query_idempotency(&self, key: &str) -> ProviderQueryResult;
}

/// Result of dispatching a single outbox item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    Sent(String),
    Failed(String, String),
    Leased(String),
    Compensated(String),
    Skipped(String),
}

/// Stateless outbox dispatcher.
#[derive(Debug)]
pub struct OutboxDispatcher;

impl OutboxDispatcher {
    /// 执行一轮调度：扫描所有 eligible items，尝试 lease → send → transition
    pub fn dispatch_round<P: ProviderDriver>(
        &self,
        outbox: &Outbox,
        provider: &P,
        worker_id: &str,
        lease_duration: Duration,
        backoff: &RetryBackoff,
        now: SystemTime,
    ) -> Vec<DispatchResult> {
        let mut results = Vec::new();

        let items = {
            let items = outbox.items.lock().unwrap();
            items.values().cloned().collect::<Vec<_>>()
        };

        for item in items {
            let item_id = item.id.clone();

            let eligible = match &item.status {
                OutboxStatus::Pending => true,
                OutboxStatus::Retry => item.next_attempt_at.is_none_or(|t| now >= t),
                OutboxStatus::Unknown => true,
                OutboxStatus::Leased => item.lease_expires_at.is_some_and(|t| now >= t),
                _ => false,
            };

            if !eligible {
                results.push(DispatchResult::Skipped(item_id));
                continue;
            }

            let mut current_item = item;

            // Handle Unknown: query provider first
            if current_item.status == OutboxStatus::Unknown {
                match provider.query_idempotency(&current_item.provider_idempotency_key) {
                    ProviderQueryResult::Sent => {
                        if let Err(e) =
                            outbox.transition(&item_id, &OutboxStatus::Unknown, &OutboxStatus::Sent)
                        {
                            results.push(DispatchResult::Failed(item_id, e.to_string()));
                            continue;
                        }
                        results.push(DispatchResult::Sent(item_id));
                        continue;
                    }
                    ProviderQueryResult::Failed => {
                        if let Err(e) = outbox.transition(
                            &item_id,
                            &OutboxStatus::Unknown,
                            &OutboxStatus::Failed,
                        ) {
                            results.push(DispatchResult::Failed(item_id, e.to_string()));
                            continue;
                        }
                        let updated = outbox
                            .item(&item_id)
                            .unwrap_or_else(|| current_item.clone());
                        if updated.compensating_action.is_some() {
                            if outbox
                                .transition(
                                    &item_id,
                                    &OutboxStatus::Failed,
                                    &OutboxStatus::Compensated,
                                )
                                .is_ok()
                            {
                                results.push(DispatchResult::Compensated(item_id));
                            } else {
                                results.push(DispatchResult::Failed(
                                    item_id,
                                    "compensate transition failed".to_string(),
                                ));
                            }
                        } else {
                            results.push(DispatchResult::Failed(
                                item_id,
                                "unknown resolved to failed".to_string(),
                            ));
                        }
                        continue;
                    }
                    ProviderQueryResult::NotFound => {
                        if let Err(e) = outbox.transition(
                            &item_id,
                            &OutboxStatus::Unknown,
                            &OutboxStatus::Retry,
                        ) {
                            results.push(DispatchResult::Failed(item_id, e.to_string()));
                            continue;
                        }
                        current_item = match outbox.item(&item_id) {
                            Some(i) => i,
                            None => {
                                results.push(DispatchResult::Failed(
                                    item_id,
                                    "item disappeared".to_string(),
                                ));
                                continue;
                            }
                        };
                    }
                }
            }

            // Try lease (Pending or Retry only at this point)
            if !outbox.lease(&item_id, worker_id, lease_duration, now) {
                results.push(DispatchResult::Skipped(item_id));
                continue;
            }

            if let Err(e) =
                outbox.transition(&item_id, &OutboxStatus::Leased, &OutboxStatus::Sending)
            {
                results.push(DispatchResult::Failed(item_id, e.to_string()));
                continue;
            }

            // Record attempt
            let lease_until = now + lease_duration;
            let attempt_id = format!(
                "{}-{}",
                item_id,
                now.duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_else(|_| Duration::from_secs(0))
                    .as_millis()
            );
            if let Err(e) = outbox.record_attempt(ChannelSendAttempt {
                attempt_id,
                outbox_item_id: item_id.clone(),
                status: OutboxStatus::Sending,
                provider_response: String::new(),
                attempted_at: now,
                lease_owner: worker_id.to_string(),
                lease_until,
                attempt_count: current_item.attempt_count,
                next_attempt_at: current_item.next_attempt_at,
                last_error_code: current_item.last_error_code.clone(),
            }) {
                warn!(error = %e, item_id = %item_id, "record_attempt failed");
            }

            match provider.send(&current_item) {
                Ok(()) => {
                    if let Err(e) =
                        outbox.transition(&item_id, &OutboxStatus::Sending, &OutboxStatus::Sent)
                    {
                        results.push(DispatchResult::Failed(item_id, e.to_string()));
                    } else {
                        results.push(DispatchResult::Sent(item_id));
                    }
                }
                Err(err) => {
                    let error_code = format!("{err:?}");
                    match backoff.compute_next(current_item.attempt_count) {
                        None => {
                            if let Err(e) = outbox.transition(
                                &item_id,
                                &OutboxStatus::Sending,
                                &OutboxStatus::Failed,
                            ) {
                                results.push(DispatchResult::Failed(item_id, e.to_string()));
                                continue;
                            }
                            let updated = outbox
                                .item(&item_id)
                                .unwrap_or_else(|| current_item.clone());
                            if updated.compensating_action.is_some() {
                                if outbox
                                    .transition(
                                        &item_id,
                                        &OutboxStatus::Failed,
                                        &OutboxStatus::Compensated,
                                    )
                                    .is_ok()
                                {
                                    results.push(DispatchResult::Compensated(item_id));
                                } else {
                                    results.push(DispatchResult::Failed(
                                        item_id,
                                        "compensate transition failed".to_string(),
                                    ));
                                }
                            } else {
                                results.push(DispatchResult::Failed(item_id, error_code));
                            }
                        }
                        Some(delay) => {
                            if let Err(e) = outbox.transition(
                                &item_id,
                                &OutboxStatus::Sending,
                                &OutboxStatus::Retry,
                            ) {
                                results.push(DispatchResult::Failed(item_id, e.to_string()));
                                continue;
                            }
                            if let Err(e) = outbox.set_retry(
                                &item_id,
                                current_item.attempt_count + 1,
                                Some(now + delay),
                                Some(error_code),
                            ) {
                                results.push(DispatchResult::Failed(item_id, e.to_string()));
                            } else {
                                results.push(DispatchResult::Failed(
                                    item_id,
                                    "retry scheduled".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        results
    }
}

/// Exponential backoff configuration.
pub struct RetryBackoff {
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub max_retries: u32,
}

impl RetryBackoff {
    pub fn compute_next(&self, attempt_count: u32) -> Option<Duration> {
        if attempt_count >= self.max_retries {
            return None;
        }
        let delay = self.base_delay * 2_u32.saturating_pow(attempt_count);
        Some(delay.min(self.max_delay))
    }
}

/// In-memory fake provider driver for tests.
#[derive(Debug)]
pub struct FakeProviderDriver {
    send_results: Mutex<HashMap<String, Result<(), ProviderError>>>,
    query_results: Mutex<HashMap<String, ProviderQueryResult>>,
    default_send: Mutex<Result<(), ProviderError>>,
    default_query: Mutex<ProviderQueryResult>,
}

impl FakeProviderDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            send_results: Mutex::new(HashMap::new()),
            query_results: Mutex::new(HashMap::new()),
            default_send: Mutex::new(Ok(())),
            default_query: Mutex::new(ProviderQueryResult::NotFound),
        }
    }

    pub fn set_send_result(&self, key: &str, result: Result<(), ProviderError>) {
        self.send_results
            .lock()
            .unwrap()
            .insert(key.to_string(), result);
    }

    pub fn set_query_result(&self, key: &str, result: ProviderQueryResult) {
        self.query_results
            .lock()
            .unwrap()
            .insert(key.to_string(), result);
    }

    pub fn set_default_send(&self, result: Result<(), ProviderError>) {
        *self.default_send.lock().unwrap() = result;
    }

    pub fn set_default_query(&self, result: ProviderQueryResult) {
        *self.default_query.lock().unwrap() = result;
    }
}

impl Default for FakeProviderDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderDriver for FakeProviderDriver {
    fn send(&self, item: &OutboxItem) -> Result<(), ProviderError> {
        let send_results = self.send_results.lock().unwrap();
        if let Some(res) = send_results.get(&item.provider_idempotency_key) {
            return res.clone();
        }
        drop(send_results);
        self.default_send.lock().unwrap().clone()
    }

    fn query_idempotency(&self, key: &str) -> ProviderQueryResult {
        let query_results = self.query_results.lock().unwrap();
        if let Some(res) = query_results.get(key) {
            return res.clone();
        }
        drop(query_results);
        self.default_query.lock().unwrap().clone()
    }
}

#[derive(Debug)]
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

    /// Enqueue an item.  Rejects duplicate `id` or already-sent `provider_idempotency_key`.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns an error if the idempotency key was already sent or the item ID is duplicate.
    pub fn enqueue(&self, item: OutboxItem) -> Result<(), &'static str> {
        let sent = self.sent_idempotency_keys.lock().unwrap();
        if sent.contains_key(&item.provider_idempotency_key) {
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
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
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

        if !matches!(
            item.status,
            OutboxStatus::Pending | OutboxStatus::Retry | OutboxStatus::Leased
        ) {
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
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns an error if the item is not found or the status does not match the expected state.
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
                Some(item.provider_idempotency_key.clone())
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
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Currently infallible; returns `Ok(())`.
    pub fn record_attempt(&self, attempt: ChannelSendAttempt) -> Result<(), &'static str> {
        #[cfg(test)]
        if TEST_FORCE_RECORD_ATTEMPT_FAIL.load(Ordering::Relaxed) {
            return Err("injected test failure");
        }
        let mut attempts = self.attempts.lock().unwrap();
        attempts.push(attempt);
        Ok(())
    }

    /// Resolve an `Unknown` item by querying the provider API.
    /// The returned status is applied to the item.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns an error if the item is not found or not in `Unknown` state.
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
            let result = query_api.query(&item.provider_idempotency_key);
            let new_status = match result {
                ProviderQueryResult::Sent => OutboxStatus::Sent,
                ProviderQueryResult::NotFound => OutboxStatus::Retry,
                ProviderQueryResult::Failed => OutboxStatus::Failed,
            };
            item.status = new_status.clone();
            if new_status == OutboxStatus::Sent {
                (Some(item.provider_idempotency_key.clone()), new_status)
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

    /// Retrieve an item by ID.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn item(&self, item_id: &str) -> Option<OutboxItem> {
        let items = self.items.lock().unwrap();
        items.get(item_id).cloned()
    }

    /// Check whether an idempotency key has already been marked as sent.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn is_idempotency_key_sent(&self, key: &str) -> bool {
        let sent = self.sent_idempotency_keys.lock().unwrap();
        sent.contains_key(key)
    }

    /// Retrieve all recorded attempts.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn attempts(&self) -> Vec<ChannelSendAttempt> {
        let attempts = self.attempts.lock().unwrap();
        attempts.clone()
    }

    /// Update retry fields for an item.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn set_retry(
        &self,
        item_id: &str,
        attempt_count: u32,
        next_attempt_at: Option<SystemTime>,
        last_error_code: Option<String>,
    ) -> Result<(), &'static str> {
        let mut items = self.items.lock().unwrap();
        let item = items.get_mut(item_id).ok_or("item not found")?;
        item.attempt_count = attempt_count;
        item.next_attempt_at = next_attempt_at;
        item.last_error_code = last_error_code;
        Ok(())
    }
}

impl Default for Outbox {
    fn default() -> Self {
        Self::new()
    }
}
