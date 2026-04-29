use super::*;
use std::thread;
use std::time::{Duration, SystemTime};

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
