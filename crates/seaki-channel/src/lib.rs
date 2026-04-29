pub mod fake_provider;
pub mod grant;
pub mod outbox;
pub mod webhook;

pub const SCHEMA_VERSION: u32 = 1;

pub use fake_provider::{BindingEntry, ChannelEvent, ChannelMessagePayload, FakeChannelProvider};
pub use grant::{ChannelResourceGrant, GrantError};
pub use outbox::{FakeProviderQueryAPI, Outbox, OutboxItem, OutboxStatus, ProviderQueryResult};
pub use webhook::{FakeWebhookVerifier, WebhookError};
