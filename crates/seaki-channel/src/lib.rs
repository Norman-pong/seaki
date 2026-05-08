pub mod broker;
pub mod fake_provider;
pub mod feishu;
pub mod grant;
pub mod ingress;
pub mod outbox;
pub mod plugin;
pub mod quarantine;
pub mod webhook;

pub const SCHEMA_VERSION: u32 = 1;

pub use broker::*;
pub use fake_provider::{BindingEntry, ChannelEvent, ChannelMessagePayload, FakeChannelProvider};
pub use grant::{ChannelResourceGrant, GrantError};
pub use ingress::*;
pub use outbox::{
    DispatchResult, FakeProviderDriver, FakeProviderQueryAPI, Outbox, OutboxDispatcher, OutboxItem,
    OutboxStatus, ProviderDriver, ProviderError, ProviderQueryResult, RetryBackoff,
};
pub use plugin::*;
pub use quarantine::*;
pub use webhook::{FakeWebhookVerifier, WebhookError};

#[cfg(test)]
mod tests;
