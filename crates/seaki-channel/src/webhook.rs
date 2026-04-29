//! Webhook verifier: signature, timestamp, replay protection.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

pub const WEBHOOK_SECRET: &str = "seaki-fake-channel-webhook-secret";

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
            ttl: Duration::from_mins(5), // 5 minutes
        }
    }

    #[must_use]
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Verify signature, timestamp and replay.
    /// On success the `event_id` is recorded to prevent replays.
    pub fn verify(
        &self,
        event_id: &str,
        raw_payload: &[u8],
        signature: &str,
        timestamp: SystemTime,
    ) -> Result<(), WebhookError> {
        self.evict_expired();

        {
            let seen = self.seen_event_ids.lock().unwrap();
            if seen.contains_key(event_id) {
                return Err(WebhookError::EventReplayed);
            }
        }

        let now = SystemTime::now();
        if now.duration_since(timestamp).unwrap_or(Duration::MAX) > self.ttl {
            return Err(WebhookError::TimestampExpired);
        }

        let expected = hmac_sha256(&self.secret, raw_payload);
        let expected_hex = hex_encode(&expected);
        if expected_hex != signature {
            return Err(WebhookError::SignatureMismatch);
        }

        let mut seen = self.seen_event_ids.lock().unwrap();
        if seen.contains_key(event_id) {
            return Err(WebhookError::EventReplayed);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn valid_signature(secret: &str, payload: &[u8]) -> String {
        hex_encode(&hmac_sha256(secret.as_bytes(), payload))
    }

    #[test]
    fn valid_payload_passes_verification() {
        let verifier = FakeWebhookVerifier::new(WEBHOOK_SECRET);
        let payload = b"{\"text\":\"hello\"}";
        let sig = valid_signature(WEBHOOK_SECRET, payload);
        let now = SystemTime::now();

        assert!(verifier.verify("evt-1", payload, &sig, now).is_ok());
    }

    #[test]
    fn signature_mismatch_returns_error() {
        let verifier = FakeWebhookVerifier::new(WEBHOOK_SECRET);
        let payload = b"{\"text\":\"hello\"}";
        let now = SystemTime::now();

        let result = verifier.verify("evt-1", payload, "bad-sig", now);
        assert_eq!(result, Err(WebhookError::SignatureMismatch));
    }

    #[test]
    fn expired_timestamp_returns_error() {
        let verifier = FakeWebhookVerifier::new(WEBHOOK_SECRET).with_ttl(Duration::from_secs(5));
        let payload = b"{\"text\":\"hello\"}";
        let sig = valid_signature(WEBHOOK_SECRET, payload);
        let old = SystemTime::now() - Duration::from_secs(10);

        let result = verifier.verify("evt-1", payload, &sig, old);
        assert_eq!(result, Err(WebhookError::TimestampExpired));
    }

    #[test]
    fn replayed_event_returns_error() {
        let verifier = FakeWebhookVerifier::new(WEBHOOK_SECRET);
        let payload = b"{\"text\":\"hello\"}";
        let sig = valid_signature(WEBHOOK_SECRET, payload);
        let now = SystemTime::now();

        verifier.verify("evt-1", payload, &sig, now).unwrap();
        let result = verifier.verify("evt-1", payload, &sig, now);
        assert_eq!(result, Err(WebhookError::EventReplayed));
    }

    #[test]
    fn concurrent_replay_allows_only_one_success() {
        let verifier = std::sync::Arc::new(FakeWebhookVerifier::new(WEBHOOK_SECRET));
        let payload = b"{\"text\":\"hello\"}";
        let sig = valid_signature(WEBHOOK_SECRET, payload);
        let now = SystemTime::now();

        let mut handles = Vec::new();
        for _ in 0..10 {
            let v = std::sync::Arc::clone(&verifier);
            let sig = sig.clone();
            handles.push(thread::spawn(move || {
                v.verify("evt-concurrent", payload, &sig, now)
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let ok_count = results.iter().filter(|r| r.is_ok()).count();
        let replay_count = results
            .iter()
            .filter(|r| **r == Err(WebhookError::EventReplayed))
            .count();
        assert_eq!(ok_count, 1);
        assert_eq!(replay_count, 9);
    }
}
