pub mod broker;
pub mod fake_provider;
pub mod grant;
pub mod outbox;
pub mod plugin;
pub mod webhook;

pub const SCHEMA_VERSION: u32 = 1;

pub use broker::*;
pub use fake_provider::{BindingEntry, ChannelEvent, ChannelMessagePayload, FakeChannelProvider};
pub use grant::{ChannelResourceGrant, GrantError};
pub use outbox::{FakeProviderQueryAPI, Outbox, OutboxItem, OutboxStatus, ProviderQueryResult};
pub use plugin::*;
pub use webhook::{FakeWebhookVerifier, WebhookError};

#[cfg(test)]
mod tests;
