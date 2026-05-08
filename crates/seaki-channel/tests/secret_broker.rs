use seaki_channel::broker::secret::{BrokerError, SecretBroker, SecretEntry};
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
