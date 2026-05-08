use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use tracing::info;

const MAX_ISSUED_TOKENS: usize = 10_000;

/// A secret stored in the broker. The raw value is never exposed to plugins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretEntry {
    pub scope: String,
    raw_value: String,
    pub description: String,
}

impl SecretEntry {
    /// Create a new secret entry.
    pub fn new(
        scope: impl Into<String>,
        raw_value: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            scope: scope.into(),
            raw_value: raw_value.into(),
            description: description.into(),
        }
    }

    /// Expose the raw secret value for a specific purpose.
    ///
    /// Callers must declare the purpose of access. This provides an audit hook
    /// point for future integration with an audit log sink.
    #[track_caller]
    pub fn expose_for(&self, purpose: &str) -> &str {
        let location = std::panic::Location::caller();
        info!(
            scope = %self.scope,
            purpose = %purpose,
            timestamp = ?SystemTime::now(),
            caller_file = %location.file(),
            caller_line = %location.line(),
            "secret_exposed"
        );
        &self.raw_value
    }
}

/// An opaque token returned to plugins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueToken {
    pub token_id: String,
    pub scope: String,
    pub expires_at: SystemTime,
}

#[derive(Debug)]
pub struct SecretBroker {
    secrets: Mutex<HashMap<String, SecretEntry>>,
    issued_tokens: Mutex<HashMap<String, OpaqueToken>>,
}

impl Default for SecretBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerError {
    SecretNotFound { scope: String },
    ScopeNotAllowed { scope: String, plugin_id: String },
    TokenExpired { token_id: String },
    TokenNotFound { token_id: String },
}

impl std::fmt::Display for BrokerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrokerError::SecretNotFound { scope } => {
                write!(f, "secret not found for scope: {scope}")
            }
            BrokerError::ScopeNotAllowed { scope, plugin_id } => {
                write!(f, "scope {scope} not allowed for plugin {plugin_id}")
            }
            BrokerError::TokenExpired { token_id } => {
                write!(f, "token expired: {token_id}")
            }
            BrokerError::TokenNotFound { token_id } => {
                write!(f, "token not found: {token_id}")
            }
        }
    }
}

impl std::error::Error for BrokerError {}

impl SecretBroker {
    pub fn new() -> Self {
        Self {
            secrets: Mutex::new(HashMap::new()),
            issued_tokens: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_secret(&self, entry: SecretEntry) {
        let mut secrets = self.secrets.lock().unwrap();
        secrets.insert(entry.scope.clone(), entry);
    }

    pub fn request_token(
        &self,
        plugin_id: &str,
        scope: &str,
        allowed_scopes: &[String],
        ttl_secs: u64,
    ) -> Result<OpaqueToken, BrokerError> {
        if !allowed_scopes.contains(&scope.to_string()) {
            return Err(BrokerError::ScopeNotAllowed {
                scope: scope.to_string(),
                plugin_id: plugin_id.to_string(),
            });
        }

        let secrets = self.secrets.lock().unwrap();
        if !secrets.contains_key(scope) {
            return Err(BrokerError::SecretNotFound {
                scope: scope.to_string(),
            });
        }
        drop(secrets);

        let token_id = format!("token_{plugin_id}_{scope}_{}", uuid::Uuid::now_v7());
        let expires_at = SystemTime::now() + Duration::from_secs(ttl_secs);
        let token = OpaqueToken {
            token_id: token_id.clone(),
            scope: scope.to_string(),
            expires_at,
        };

        let mut issued_tokens = self.issued_tokens.lock().unwrap();

        // Periodic cleanup of expired tokens when approaching capacity.
        if issued_tokens.len().is_multiple_of(100) || issued_tokens.len() >= MAX_ISSUED_TOKENS {
            let now = SystemTime::now();
            issued_tokens.retain(|_, t| now <= t.expires_at);
        }

        // Hard limit: evict arbitrary entries if still over capacity after cleanup.
        while issued_tokens.len() >= MAX_ISSUED_TOKENS {
            let key_to_remove = issued_tokens.keys().next().cloned();
            if let Some(key) = key_to_remove {
                issued_tokens.remove(&key);
            } else {
                break;
            }
        }

        issued_tokens.insert(token_id, token.clone());

        Ok(token)
    }

    pub fn resolve_token(&self, token_id: &str) -> Result<SecretEntry, BrokerError> {
        let mut issued_tokens = self.issued_tokens.lock().unwrap();
        let token = issued_tokens
            .get(token_id)
            .ok_or_else(|| BrokerError::TokenNotFound {
                token_id: token_id.to_string(),
            })?
            .clone();

        if SystemTime::now() > token.expires_at {
            issued_tokens.remove(token_id);
            return Err(BrokerError::TokenExpired {
                token_id: token.token_id,
            });
        }

        drop(issued_tokens);

        let secrets = self.secrets.lock().unwrap();
        secrets
            .get(&token.scope)
            .cloned()
            .ok_or(BrokerError::SecretNotFound { scope: token.scope })
    }

    pub fn revoke_token(&self, token_id: &str) -> bool {
        let mut issued_tokens = self.issued_tokens.lock().unwrap();
        issued_tokens.remove(token_id).is_some()
    }

    pub fn list_scopes(&self) -> Vec<String> {
        let secrets = self.secrets.lock().unwrap();
        secrets.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{SecretBroker, SecretEntry};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use tracing::field::{Field, Visit};
    use tracing::{span, Event, Id, Metadata, Subscriber};

    #[test]
    fn token_id_no_collisions_under_high_concurrency() {
        let broker = Arc::new(SecretBroker::new());
        broker.register_secret(SecretEntry::new("slack", "secret", "desc"));

        let thread_count = 100;
        let tokens_per_thread = 10;
        let barrier = Arc::new(Barrier::new(thread_count));

        let handles: Vec<_> = (0..thread_count)
            .map(|i| {
                let broker = Arc::clone(&broker);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let mut tokens = Vec::with_capacity(tokens_per_thread);
                    for _ in 0..tokens_per_thread {
                        let token = broker
                            .request_token(
                                &format!("plugin-{i}"),
                                "slack",
                                &["slack".to_string()],
                                3600,
                            )
                            .unwrap();
                        tokens.push(token.token_id);
                    }
                    tokens
                })
            })
            .collect();

        let mut all_tokens = std::collections::HashSet::new();
        for handle in handles {
            for token_id in handle.join().unwrap() {
                assert!(
                    all_tokens.insert(token_id.clone()),
                    "duplicate token_id: {token_id}"
                );
            }
        }
        assert_eq!(all_tokens.len(), thread_count * tokens_per_thread);
    }

    type EventList = Vec<Vec<(String, String)>>;

    #[derive(Debug, Clone, Default)]
    struct AuditCapture {
        events: Arc<Mutex<EventList>>,
    }

    struct AuditVisitor {
        fields: Vec<(String, String)>,
    }

    impl Visit for AuditVisitor {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.fields
                .push((field.name().to_string(), format!("{value:?}")));
        }
    }

    impl Subscriber for AuditCapture {
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            true
        }
        fn new_span(&self, _span: &span::Attributes<'_>) -> Id {
            Id::from_u64(1)
        }
        fn record(&self, _span: &Id, _values: &span::Record<'_>) {}
        fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
        fn event(&self, event: &Event<'_>) {
            let mut visitor = AuditVisitor { fields: Vec::new() };
            event.record(&mut visitor);
            self.events.lock().unwrap().push(visitor.fields);
        }
        fn enter(&self, _span: &Id) {}
        fn exit(&self, _span: &Id) {}
    }

    #[test]
    fn expose_for_records_audit_log() {
        let capture = AuditCapture::default();
        let events = capture.events.clone();
        tracing::subscriber::with_default(capture, || {
            let entry = SecretEntry::new("scope1", "secret1", "desc");
            let value = entry.expose_for("test_purpose");
            assert_eq!(value, "secret1");
        });

        let logs = events.lock().unwrap();
        assert_eq!(logs.len(), 1, "expected exactly one audit event");
        let fields: std::collections::HashMap<String, String> = logs[0].iter().cloned().collect();
        assert_eq!(fields.get("message"), Some(&"secret_exposed".to_string()));
        assert_eq!(fields.get("scope"), Some(&"scope1".to_string()));
        assert_eq!(fields.get("purpose"), Some(&"test_purpose".to_string()));
        assert!(fields.contains_key("timestamp"), "expected timestamp field");
        assert!(
            fields.contains_key("caller_file"),
            "expected caller_file field"
        );
        assert!(
            fields.contains_key("caller_line"),
            "expected caller_line field"
        );
    }
}
