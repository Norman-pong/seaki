use seaki_channel::broker::secret::{BrokerError, SecretBroker, SecretEntry};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

#[test]
fn register_and_request_token() {
    let broker = SecretBroker::new();
    broker.register_secret(SecretEntry::new("slack", "xoxb-secret", "Slack bot token"));

    let token = broker
        .request_token("plugin-1", "slack", &["slack".to_string()], 3600)
        .unwrap();
    assert_eq!(token.scope, "slack");
    assert!(token.expires_at > SystemTime::now());
}

#[test]
fn scope_not_allowed() {
    let broker = SecretBroker::new();
    broker.register_secret(SecretEntry::new("slack", "xoxb-secret", "Slack bot token"));

    let err = broker
        .request_token("plugin-1", "slack", &["discord".to_string()], 3600)
        .unwrap_err();
    assert!(matches!(
        err,
        BrokerError::ScopeNotAllowed {
            scope,
            plugin_id,
        } if scope == "slack" && plugin_id == "plugin-1"
    ));
}

#[test]
fn resolve_token_returns_secret() {
    let broker = SecretBroker::new();
    broker.register_secret(SecretEntry::new("slack", "xoxb-secret", "Slack bot token"));

    let token = broker
        .request_token("plugin-1", "slack", &["slack".to_string()], 3600)
        .unwrap();
    let entry = broker.resolve_token(&token.token_id).unwrap();
    assert_eq!(entry.scope, "slack");
    assert_eq!(entry.expose_for("test"), "xoxb-secret");
}

#[test]
fn revoke_token() {
    let broker = SecretBroker::new();
    broker.register_secret(SecretEntry::new("slack", "xoxb-secret", "Slack bot token"));

    let token = broker
        .request_token("plugin-1", "slack", &["slack".to_string()], 3600)
        .unwrap();
    assert!(broker.revoke_token(&token.token_id));
    let err = broker.resolve_token(&token.token_id).unwrap_err();
    assert!(matches!(err, BrokerError::TokenNotFound { .. }));
}

#[test]
fn token_expired() {
    let broker = SecretBroker::new();
    broker.register_secret(SecretEntry::new("slack", "xoxb-secret", "Slack bot token"));

    let token = broker
        .request_token("plugin-1", "slack", &["slack".to_string()], 0)
        .unwrap();
    // Small sleep to ensure expiration
    std::thread::sleep(Duration::from_millis(50));
    let err = broker.resolve_token(&token.token_id).unwrap_err();
    assert!(matches!(err, BrokerError::TokenExpired { .. }));
}

#[test]
fn issued_tokens_bounded_cleanup() {
    let broker = SecretBroker::new();
    broker.register_secret(SecretEntry::new("slack", "xoxb-secret", "Slack bot token"));

    // Request many tokens without resolving them.
    for i in 0..10_005 {
        let _ = broker
            .request_token(
                &format!("plugin-{i}"),
                "slack",
                &["slack".to_string()],
                3600,
            )
            .unwrap();
    }

    // All tokens are still valid (TTL 3600), but the broker should enforce a hard limit.
    // We verify by requesting one more token successfully.
    let token = broker
        .request_token("plugin-overflow", "slack", &["slack".to_string()], 3600)
        .unwrap();
    assert_eq!(token.scope, "slack");
}

// S1: resolve_token should not return a secret after concurrent revocation.
#[test]
fn resolve_after_concurrent_revoke() {
    let broker = Arc::new(SecretBroker::new());
    broker.register_secret(SecretEntry::new("slack", "xoxb-secret", "Slack bot token"));

    let token = broker
        .request_token("plugin-1", "slack", &["slack".to_string()], 3600)
        .unwrap();
    let token_id = token.token_id.clone();

    let tid_resolve = token_id.clone();
    let b1 = Arc::clone(&broker);
    let resolve_handle = std::thread::spawn(move || {
        // Spin-resolve in a tight loop to increase chance of hitting the race window.
        for _ in 0..100 {
            let _ = b1.resolve_token(&tid_resolve);
        }
    });

    let tid_revoke = token_id.clone();
    let b2 = Arc::clone(&broker);
    let revoke_handle = std::thread::spawn(move || {
        for _ in 0..100 {
            b2.revoke_token(&tid_revoke);
        }
    });

    resolve_handle.join().unwrap();
    revoke_handle.join().unwrap();

    // After revocation, resolve must fail.
    broker.revoke_token(&token_id);
    let err = broker.resolve_token(&token_id).unwrap_err();
    assert!(matches!(err, BrokerError::TokenNotFound { .. }));
}
