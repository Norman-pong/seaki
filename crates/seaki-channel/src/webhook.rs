//! Webhook verifier: signature, timestamp, replay protection.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

/// Constant-time comparison of two hex strings to mitigate timing attacks (S6).
pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (ca, cb) in a.bytes().zip(b.bytes()) {
        result |= ca ^ cb;
    }
    result == 0
}

pub const WEBHOOK_SECRET: &str = "seaki-fake-channel-webhook-secret";
const MAX_SEEN_EVENT_IDS: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookError {
    SignatureMismatch,
    TimestampExpired,
    EventReplayed,
}

impl std::fmt::Display for WebhookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignatureMismatch => write!(f, "SIGNATURE_MISMATCH"),
            Self::TimestampExpired => write!(f, "TIMESTAMP_EXPIRED"),
            Self::EventReplayed => write!(f, "EVENT_REPLAYED"),
        }
    }
}

impl std::error::Error for WebhookError {}

/// Trait for webhook payload verification.
pub trait WebhookVerifier: Send + Sync {
    /// Verify a webhook payload.
    fn verify(
        &self,
        event_id: &str,
        raw_payload: &[u8],
        signature: &str,
        timestamp: SystemTime,
    ) -> Result<(), WebhookError>;
}

impl WebhookVerifier for FakeWebhookVerifier {
    fn verify(
        &self,
        event_id: &str,
        raw_payload: &[u8],
        signature: &str,
        timestamp: SystemTime,
    ) -> Result<(), WebhookError> {
        self.verify(event_id, raw_payload, signature, timestamp)
    }
}

/// Simple in-memory HMAC-SHA256 (RFC 2104) using only `sha2`.
#[must_use]
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let block_size = 64usize;
    let mut k = key.to_vec();
    if k.len() > block_size {
        k = Sha256::digest(&k).to_vec();
    }
    if k.len() < block_size {
        k.resize(block_size, 0);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..block_size {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

/// Verifies webhook payloads using HMAC-SHA256, timestamp tolerance and
/// idempotency-based replay protection.
#[derive(Debug)]
pub struct FakeWebhookVerifier {
    secret: Vec<u8>,
    seen_event_ids: Mutex<HashMap<String, SystemTime>>,
    ttl: Duration,
}

impl FakeWebhookVerifier {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into().into_bytes(),
            seen_event_ids: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(300), // 5 minutes
        }
    }

    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Verify signature, timestamp and replay.
    /// On success the `event_id` is recorded to prevent replays.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    ///
    /// # Errors
    ///
    /// Returns `WebhookError` if the event is replayed, timestamp expired, or signature invalid.
    pub fn verify(
        &self,
        event_id: &str,
        raw_payload: &[u8],
        signature: &str,
        timestamp: SystemTime,
    ) -> Result<(), WebhookError> {
        self.evict_expired();

        let now = SystemTime::now();
        if now.duration_since(timestamp).unwrap_or(Duration::MAX) > self.ttl {
            return Err(WebhookError::TimestampExpired);
        }

        // S2 fix: perform HMAC verification inside the write lock to prevent
        // concurrent requests from executing expensive HMAC calculations for
        // the same new event. Write lock also naturally serializes replay checks.
        let mut seen = self.seen_event_ids.lock().unwrap();
        if seen.contains_key(event_id) {
            return Err(WebhookError::EventReplayed);
        }

        let expected = hmac_sha256(&self.secret, raw_payload);
        let expected_hex = hex_encode(&expected);
        if !constant_time_eq(&expected_hex, signature) {
            return Err(WebhookError::SignatureMismatch);
        }

        // S6 fix: use constant-time comparison above instead of !=.

        // Double-check replay under write lock (another thread may have
        // inserted while we computed HMAC).
        if seen.contains_key(event_id) {
            return Err(WebhookError::EventReplayed);
        }
        // Enforce size bound by evicting the oldest entry when at capacity.
        if seen.len() >= MAX_SEEN_EVENT_IDS {
            let oldest = seen.iter().min_by_key(|(_, t)| *t).map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                seen.remove(&k);
            }
        }
        seen.insert(event_id.to_string(), now);
        Ok(())
    }

    fn evict_expired(&self) {
        let now = SystemTime::now();
        let mut seen = self.seen_event_ids.lock().unwrap();
        seen.retain(|_, &mut t| now.duration_since(t).unwrap_or(Duration::MAX) <= self.ttl);
    }
}
