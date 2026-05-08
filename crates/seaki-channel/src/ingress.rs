//! Ingress normalizer: verify webhook, resolve identity, produce normalized ChannelEvent.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

const MAX_AUDIT_LOG_SIZE: usize = 10_000;

use crate::fake_provider::{ChannelEvent, ChannelMessagePayload};
use crate::webhook::{WebhookError, WebhookVerifier};

/// A resolved identity from the binding table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIdentity {
    pub seaki_workspace_id: String,
    pub seaki_actor_id: String,
    pub workspace_role: String,
}

/// Policy for handling unmapped users.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnmappedUserPolicy {
    Reject,
    Guest,
}

/// Trait for identity resolution.
pub trait IdentityResolver: Send + Sync {
    /// Resolve a provider identity to a Seaki identity.
    fn resolve(
        &self,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
    ) -> Option<ResolvedIdentity>;
}

/// In-memory identity resolver backed by a binding table.
#[derive(Debug)]
pub struct InMemoryIdentityResolver {
    bindings: Mutex<HashMap<(String, String, String), ResolvedIdentity>>,
}

impl InMemoryIdentityResolver {
    /// Create a new empty resolver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: Mutex::new(HashMap::new()),
        }
    }

    /// Insert or overwrite a binding.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn upsert(
        &self,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
        identity: ResolvedIdentity,
    ) {
        let mut bindings = self.bindings.lock().unwrap();
        bindings.insert(
            (
                provider_tenant_id.to_string(),
                channel_binding_id.to_string(),
                provider_user_id.to_string(),
            ),
            identity,
        );
    }

    /// Remove a binding and return the old identity if present.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn remove(
        &self,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
    ) -> Option<ResolvedIdentity> {
        let mut bindings = self.bindings.lock().unwrap();
        bindings.remove(&(
            provider_tenant_id.to_string(),
            channel_binding_id.to_string(),
            provider_user_id.to_string(),
        ))
    }
}

impl Default for InMemoryIdentityResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl IdentityResolver for InMemoryIdentityResolver {
    fn resolve(
        &self,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
    ) -> Option<ResolvedIdentity> {
        let bindings = self.bindings.lock().unwrap();
        bindings
            .get(&(
                provider_tenant_id.to_string(),
                channel_binding_id.to_string(),
                provider_user_id.to_string(),
            ))
            .cloned()
    }
}

/// Errors that can occur during ingress normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngressError {
    Webhook(WebhookError),
    IdentityNotFound,
    InvalidPayload(String),
    ReplayDetected,
}

impl std::fmt::Display for IngressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Webhook(e) => write!(f, "webhook verification failed: {e}"),
            Self::IdentityNotFound => write!(f, "identity not found in binding table"),
            Self::InvalidPayload(msg) => write!(f, "invalid payload: {msg}"),
            Self::ReplayDetected => write!(f, "event replay detected"),
        }
    }
}

impl std::error::Error for IngressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Webhook(e) => Some(e),
            _ => None,
        }
    }
}

/// Result of an ingress attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngressResult {
    Accepted,
    RejectedSignature,
    RejectedExpired,
    RejectedReplay,
    RejectedUnmapped,
}

/// Audit record for each ingress attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngressAuditRecord {
    pub event_id: String,
    pub provider_tenant_id: String,
    pub channel_binding_id: String,
    pub provider_user_id: String,
    pub seaki_actor_id: Option<String>,
    pub result: IngressResult,
    pub timestamp: SystemTime,
}

/// Normalizes raw webhook payloads into standard [`ChannelEvent`]s.
#[derive(Debug)]
pub struct IngressNormalizer<V: WebhookVerifier, R: IdentityResolver> {
    verifier: V,
    resolver: R,
    unmapped_policy: UnmappedUserPolicy,
    audit_log: Mutex<Vec<IngressAuditRecord>>,
}

impl<V: WebhookVerifier, R: IdentityResolver> IngressNormalizer<V, R> {
    /// Create a new normalizer.
    #[must_use]
    pub fn new(verifier: V, resolver: R, unmapped_policy: UnmappedUserPolicy) -> Self {
        Self {
            verifier,
            resolver,
            unmapped_policy,
            audit_log: Mutex::new(Vec::new()),
        }
    }

    /// Main entry: verify webhook, resolve identity, normalize event.
    ///
    /// # Errors
    ///
    /// Returns [`IngressError`] if verification fails, identity is not found,
    /// or a replay is detected.
    #[allow(clippy::too_many_arguments)]
    pub fn normalize(
        &self,
        raw_payload: &[u8],
        signature: &str,
        timestamp: SystemTime,
        event_id: &str,
        event_type: &str,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
        payload: ChannelMessagePayload,
    ) -> Result<ChannelEvent, IngressError> {
        // Step 1: Verify webhook
        let verified_at = SystemTime::now();
        match self
            .verifier
            .verify(event_id, raw_payload, signature, timestamp)
        {
            Ok(()) => {}
            Err(WebhookError::SignatureMismatch) => {
                self.record_audit(
                    event_id,
                    provider_tenant_id,
                    channel_binding_id,
                    provider_user_id,
                    None,
                    IngressResult::RejectedSignature,
                );
                return Err(IngressError::Webhook(WebhookError::SignatureMismatch));
            }
            Err(WebhookError::TimestampExpired) => {
                self.record_audit(
                    event_id,
                    provider_tenant_id,
                    channel_binding_id,
                    provider_user_id,
                    None,
                    IngressResult::RejectedExpired,
                );
                return Err(IngressError::Webhook(WebhookError::TimestampExpired));
            }
            Err(WebhookError::EventReplayed) => {
                self.record_audit(
                    event_id,
                    provider_tenant_id,
                    channel_binding_id,
                    provider_user_id,
                    None,
                    IngressResult::RejectedReplay,
                );
                return Err(IngressError::ReplayDetected);
            }
        }

        // Step 2: Resolve identity
        let identity =
            self.resolver
                .resolve(provider_tenant_id, channel_binding_id, provider_user_id);
        let (seaki_workspace_id, seaki_actor_id, workspace_role) = match identity {
            Some(id) => (id.seaki_workspace_id, id.seaki_actor_id, id.workspace_role),
            None => match self.unmapped_policy {
                UnmappedUserPolicy::Reject => {
                    self.record_audit(
                        event_id,
                        provider_tenant_id,
                        channel_binding_id,
                        provider_user_id,
                        None,
                        IngressResult::RejectedUnmapped,
                    );
                    return Err(IngressError::IdentityNotFound);
                }
                UnmappedUserPolicy::Guest => {
                    let guest_actor = format!("guest:{provider_user_id}");
                    ("default".to_string(), guest_actor, "guest".to_string())
                }
            },
        };

        let normalized_at = SystemTime::now();
        let channel_scope = format!(
            "workspace:{seaki_workspace_id}/channel:{channel_binding_id}/user:{provider_user_id}"
        );

        self.record_audit(
            event_id,
            provider_tenant_id,
            channel_binding_id,
            provider_user_id,
            Some(&seaki_actor_id),
            IngressResult::Accepted,
        );

        Ok(ChannelEvent {
            event_id: event_id.to_string(),
            event_type: event_type.to_string(),
            provider_tenant_id: provider_tenant_id.to_string(),
            channel_binding_id: channel_binding_id.to_string(),
            provider_user_id: provider_user_id.to_string(),
            payload,
            timestamp,
            seaki_workspace_id,
            seaki_actor_id,
            workspace_role,
            channel_scope,
            signature_verified_at: verified_at,
            normalized_at,
        })
    }

    /// Return a clone of the full audit log.
    pub fn audit_log(&self) -> Vec<IngressAuditRecord> {
        self.audit_log.lock().unwrap().clone()
    }

    /// Return audit records for a specific event id.
    pub fn audit_for_event(&self, event_id: &str) -> Vec<IngressAuditRecord> {
        self.audit_log
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.event_id == event_id)
            .cloned()
            .collect()
    }

    fn record_audit(
        &self,
        event_id: &str,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
        seaki_actor_id: Option<&str>,
        result: IngressResult,
    ) {
        let record = IngressAuditRecord {
            event_id: event_id.to_string(),
            provider_tenant_id: provider_tenant_id.to_string(),
            channel_binding_id: channel_binding_id.to_string(),
            provider_user_id: provider_user_id.to_string(),
            seaki_actor_id: seaki_actor_id.map(|s| s.to_string()),
            result,
            timestamp: SystemTime::now(),
        };
        let mut log = self.audit_log.lock().unwrap();
        if log.len() >= MAX_AUDIT_LOG_SIZE {
            log.remove(0);
        }
        log.push(record);
    }
}
