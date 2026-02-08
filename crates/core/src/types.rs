use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PricingTier {
    Free,
    Pro,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountTier {
    Free,
    Pro,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    Active,
    Suspended,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChannelStatus {
    Active,
    Paused,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignalUrgency {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignalStatus {
    Active,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionStatus {
    Active,
    Paused,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebhookStatus {
    Active,
    Paused,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Pending,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryMode {
    Agent,
    Webhook,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyOwner {
    Publisher,
    Subscriber,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyStatus {
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publisher {
    pub id: String,
    pub name: String,
    pub email: String,
    pub stripe_customer_id: Option<String>,
    pub stripe_connect_id: Option<String>,
    pub tier: AccountTier,
    pub status: AccountStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscriber {
    pub id: String,
    pub name: String,
    pub email: String,
    pub webhook_secret: String,
    pub stripe_customer_id: Option<String>,
    pub tier: AccountTier,
    pub status: AccountStatus,
    pub delivery_mode: DeliveryMode,
    pub agent_last_connected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub publisher_id: String,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub pricing_tier: PricingTier,
    pub price_cents: i32,
    pub status: ChannelStatus,
    pub is_public: bool,
    pub signal_count: i32,
    pub subscriber_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub channel_id: String,
    pub title: String,
    pub body: String,
    pub urgency: SignalUrgency,
    pub metadata: serde_json::Value,
    pub delivery_count: i32,
    pub delivered_count: i32,
    pub failed_count: i32,
    pub status: SignalStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    pub subscriber_id: String,
    pub url: String,
    pub name: String,
    pub token: Option<String>,
    pub status: WebhookStatus,
    pub failure_count: i32,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub subscriber_id: String,
    pub channel_id: String,
    pub webhook_id: Option<String>,
    pub status: SubscriptionStatus,
    pub stripe_subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delivery {
    pub id: String,
    pub signal_id: String,
    pub subscription_id: String,
    pub webhook_id: Option<String>,
    pub delivery_mode: DeliveryMode,
    pub attempt: i32,
    pub status: DeliveryStatus,
    pub status_code: Option<i32>,
    pub error_message: Option<String>,
    pub latency_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub owner_type: ApiKeyOwner,
    pub owner_id: String,
    pub name: Option<String>,
    pub scopes: Vec<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: ApiKeyStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub id: String,
    pub delivery_id: String,
    pub signal_id: String,
    pub subscription_id: String,
    pub payload: serde_json::Value,
    pub error_history: serde_json::Value,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryJob {
    pub signal_id: String,
    pub subscription_id: String,
    pub webhook_id: Option<String>,
    pub attempt: i32,
}
