//! Fake channel provider: webhook, binding, event normalization.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::webhook::{FakeWebhookVerifier, WebhookError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMessagePayload {
    pub text: String,
    pub attachments: Vec<super::grant::ChannelAttachmentRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelEvent {
    pub event_id: String,
    pub event_type: String,
    pub provider_tenant_id: String,
    pub channel_binding_id: String,
    pub provider_user_id: String,
    pub payload: ChannelMessagePayload,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingEntry {
    pub provider_tenant_id: String,
    pub channel_binding_id: String,
    pub provider_user_id: String,
    pub seaki_actor_id: String,
    pub workspace_role: String,
}

/// In-memory fake channel provider.
///
/// Binding table maps `(provider_tenant_id, channel_binding_id, provider_user_id)`
/// to a `BindingEntry`.  The provider itself **cannot** declare `seaki_actor_id`;
/// it is resolved exclusively through the binding table.
pub struct FakeChannelProvider {
    verifier: FakeWebhookVerifier,
    bindings: Mutex<HashMap<(String, String, String), BindingEntry>>,
}

impl FakeChannelProvider {
    pub fn new() -> Self {
        Self {
            verifier: FakeWebhookVerifier::new(super::webhook::WEBHOOK_SECRET),
            bindings: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_verifier(verifier: FakeWebhookVerifier) -> Self {
        Self {
            verifier,
            bindings: Mutex::new(HashMap::new()),
        }
    }

    /// Add or overwrite a binding entry.
    pub fn upsert_binding(&self, entry: BindingEntry) {
        let mut bindings = self.bindings.lock().unwrap();
        bindings.insert(
            (
                entry.provider_tenant_id.clone(),
                entry.channel_binding_id.clone(),
                entry.provider_user_id.clone(),
            ),
            entry,
        );
    }

    /// Remove a binding entry.
    pub fn remove_binding(
        &self,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
    ) -> Option<BindingEntry> {
        let mut bindings = self.bindings.lock().unwrap();
        bindings.remove(&(
            provider_tenant_id.to_string(),
            channel_binding_id.to_string(),
            provider_user_id.to_string(),
        ))
    }

    /// Resolve provider identity to Seaki actor via binding table.
    pub fn resolve_actor(
        &self,
        provider_tenant_id: &str,
        channel_binding_id: &str,
        provider_user_id: &str,
    ) -> Option<BindingEntry> {
        let bindings = self.bindings.lock().unwrap();
        bindings
            .get(&(
                provider_tenant_id.to_string(),
                channel_binding_id.to_string(),
                provider_user_id.to_string(),
            ))
            .cloned()
    }

    /// Submit a raw webhook payload after verification.
    ///
    /// The `seaki_actor_id` is **not** accepted as an argument; it is derived
    /// from the binding table.  If no binding exists the submission fails.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_event(
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
    ) -> Result<ChannelEvent, WebhookError> {
        self.verifier
            .verify(event_id, raw_payload, signature, timestamp)?;

        let binding = self
            .resolve_actor(provider_tenant_id, channel_binding_id, provider_user_id)
            .ok_or(WebhookError::SignatureMismatch)?;

        Ok(ChannelEvent {
            event_id: event_id.to_string(),
            event_type: event_type.to_string(),
            provider_tenant_id: binding.provider_tenant_id,
            channel_binding_id: binding.channel_binding_id,
            provider_user_id: binding.provider_user_id,
            payload,
            timestamp,
        })
    }
}

impl Default for FakeChannelProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webhook::WEBHOOK_SECRET;
    use crate::webhook::{hex_encode, hmac_sha256};

    fn sig(payload: &[u8]) -> String {
        hex_encode(&hmac_sha256(WEBHOOK_SECRET.as_bytes(), payload))
    }

    fn binding() -> BindingEntry {
        BindingEntry {
            provider_tenant_id: "tenant-1".to_string(),
            channel_binding_id: "bind-1".to_string(),
            provider_user_id: "user-1".to_string(),
            seaki_actor_id: "actor-1".to_string(),
            workspace_role: "member".to_string(),
        }
    }

    #[test]
    fn provider_cannot_declare_seaki_actor_id() {
        let provider = FakeChannelProvider::new();
        provider.upsert_binding(binding());
        let payload = b"{\"text\":\"hi\"}";
        let now = SystemTime::now();

        // submit_event does NOT accept seaki_actor_id; it is resolved from binding.
        let event = provider
            .submit_event(
                payload,
                &sig(payload),
                now,
                "evt-1",
                "channel.message.v1",
                "tenant-1",
                "bind-1",
                "user-1",
                ChannelMessagePayload {
                    text: "hi".to_string(),
                    attachments: Vec::new(),
                },
            )
            .expect("submit should succeed");

        // The returned event does NOT expose seaki_actor_id directly.
        assert_eq!(event.provider_tenant_id, "tenant-1");
        assert_eq!(event.provider_user_id, "user-1");
    }

    #[test]
    fn unbound_identity_rejected() {
        let provider = FakeChannelProvider::new();
        // no binding inserted
        let payload = b"{\"text\":\"hi\"}";
        let now = SystemTime::now();

        let result = provider.submit_event(
            payload,
            &sig(payload),
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            ChannelMessagePayload {
                text: "hi".to_string(),
                attachments: Vec::new(),
            },
        );
        assert_eq!(result, Err(WebhookError::SignatureMismatch));
    }

    #[test]
    fn binding_crud_works() {
        let provider = FakeChannelProvider::new();
        let b = binding();
        provider.upsert_binding(b.clone());

        let resolved = provider.resolve_actor("tenant-1", "bind-1", "user-1");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().seaki_actor_id, "actor-1");

        let removed = provider.remove_binding("tenant-1", "bind-1", "user-1");
        assert!(removed.is_some());

        let resolved2 = provider.resolve_actor("tenant-1", "bind-1", "user-1");
        assert!(resolved2.is_none());
    }

    #[test]
    fn webhook_verification_failure_propagated() {
        let provider = FakeChannelProvider::new();
        provider.upsert_binding(binding());
        let payload = b"{\"text\":\"hi\"}";
        let now = SystemTime::now();

        let result = provider.submit_event(
            payload,
            "bad-sig",
            now,
            "evt-1",
            "channel.message.v1",
            "tenant-1",
            "bind-1",
            "user-1",
            ChannelMessagePayload {
                text: "hi".to_string(),
                attachments: Vec::new(),
            },
        );
        assert_eq!(result, Err(WebhookError::SignatureMismatch));
    }
}
