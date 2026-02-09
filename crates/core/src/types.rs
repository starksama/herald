//! Domain types for Herald.
//!
//! This module contains the core domain types used throughout Herald.
//! These types are database-agnostic and use only serde for serialization.
//!
//! Note: `crates/db/src/models.rs` contains parallel definitions with sqlx
//! derives for database operations. When modifying types here, ensure the
//! corresponding db model is updated as well.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Pricing tier for channels.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PricingTier {
    Free,
    Pro,
    Enterprise,
}

/// Account tier for publishers and subscribers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountTier {
    Free,
    Pro,
    Enterprise,
}

/// Account lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    Active,
    Suspended,
    Deleted,
}

/// Channel lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChannelStatus {
    Active,
    Paused,
    Deleted,
}

/// Signal urgency level, affects delivery priority.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignalUrgency {
    Low,
    Normal,
    High,
    Critical,
}

/// Signal lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SignalStatus {
    Active,
    Deleted,
}

/// Subscription lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionStatus {
    Active,
    Paused,
    Canceled,
}

/// Webhook endpoint status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebhookStatus {
    Active,
    Paused,
    /// Automatically disabled after repeated failures.
    Disabled,
}

/// Delivery attempt status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Pending,
    Success,
    Failed,
}

/// How signals are delivered to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryMode {
    /// Via persistent WebSocket tunnel (herald-agent).
    Agent,
    /// Via HTTP POST to subscriber's endpoint.
    Webhook,
}

/// API key owner type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyOwner {
    Publisher,
    Subscriber,
}

/// API key lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyStatus {
    Active,
    Revoked,
    Expired,
}

/// A publisher who creates channels and sends signals.
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

/// A subscriber who receives signals from subscribed channels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscriber {
    pub id: String,
    pub name: String,
    pub email: String,
    /// Secret used to sign webhook payloads for this subscriber.
    pub webhook_secret: String,
    pub stripe_customer_id: Option<String>,
    pub tier: AccountTier,
    pub status: AccountStatus,
    pub delivery_mode: DeliveryMode,
    /// Last time the subscriber's agent connected via tunnel.
    pub agent_last_connected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A channel that publishers use to broadcast signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub publisher_id: String,
    /// URL-friendly identifier (e.g., "my-alerts").
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub pricing_tier: PricingTier,
    /// Monthly subscription price in cents.
    pub price_cents: i32,
    pub status: ChannelStatus,
    /// Whether the channel is listed in the marketplace.
    pub is_public: bool,
    pub signal_count: i32,
    pub subscriber_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A signal (notification) sent through a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub channel_id: String,
    pub title: String,
    pub body: String,
    pub urgency: SignalUrgency,
    /// Arbitrary JSON metadata attached to the signal.
    pub metadata: serde_json::Value,
    /// Total delivery attempts across all subscribers.
    pub delivery_count: i32,
    pub delivered_count: i32,
    pub failed_count: i32,
    pub status: SignalStatus,
    pub created_at: DateTime<Utc>,
}

/// A webhook endpoint configured by a subscriber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    pub subscriber_id: String,
    pub url: String,
    pub name: String,
    /// Optional bearer token sent in Authorization header.
    pub token: Option<String>,
    pub status: WebhookStatus,
    /// Consecutive failure count (resets on success).
    pub failure_count: i32,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A subscription linking a subscriber to a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub subscriber_id: String,
    pub channel_id: String,
    /// If set, signals are delivered to this webhook instead of the default.
    pub webhook_id: Option<String>,
    pub status: SubscriptionStatus,
    pub stripe_subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single delivery attempt of a signal to a subscriber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delivery {
    pub id: String,
    pub signal_id: String,
    pub subscription_id: String,
    pub webhook_id: Option<String>,
    pub delivery_mode: DeliveryMode,
    /// Attempt number (1 for first attempt, increments on retry).
    pub attempt: i32,
    pub status: DeliveryStatus,
    /// HTTP status code from webhook response (if applicable).
    pub status_code: Option<i32>,
    pub error_message: Option<String>,
    /// Round-trip latency in milliseconds.
    pub latency_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An API key for authenticating publishers or subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    /// SHA-256 hash of the raw key (raw key never stored).
    pub key_hash: String,
    /// First 12 chars of the key for identification (e.g., "hld_pub_xxxx").
    pub key_prefix: String,
    pub owner_type: ApiKeyOwner,
    pub owner_id: String,
    pub name: Option<String>,
    /// Permission scopes (e.g., ["signals:write", "channels:read"]).
    pub scopes: Vec<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: ApiKeyStatus,
    pub created_at: DateTime<Utc>,
}

/// Failed delivery stored for manual inspection and retry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub id: String,
    pub delivery_id: String,
    pub signal_id: String,
    pub subscription_id: String,
    /// Original signal payload for replay.
    pub payload: serde_json::Value,
    /// Array of error messages from each failed attempt.
    pub error_history: serde_json::Value,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Job payload for the delivery worker queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryJob {
    pub signal_id: String,
    pub subscription_id: String,
    pub webhook_id: Option<String>,
    pub attempt: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Enum Serialization Tests (lowercase rename_all)
    // ============================================================

    #[test]
    fn test_pricing_tier_serialization() {
        assert_eq!(serde_json::to_string(&PricingTier::Free).unwrap(), "\"free\"");
        assert_eq!(serde_json::to_string(&PricingTier::Pro).unwrap(), "\"pro\"");
        assert_eq!(serde_json::to_string(&PricingTier::Enterprise).unwrap(), "\"enterprise\"");
    }

    #[test]
    fn test_pricing_tier_deserialization() {
        assert_eq!(serde_json::from_str::<PricingTier>("\"free\"").unwrap(), PricingTier::Free);
        assert_eq!(serde_json::from_str::<PricingTier>("\"pro\"").unwrap(), PricingTier::Pro);
        assert_eq!(serde_json::from_str::<PricingTier>("\"enterprise\"").unwrap(), PricingTier::Enterprise);
    }

    #[test]
    fn test_account_tier_serialization() {
        assert_eq!(serde_json::to_string(&AccountTier::Free).unwrap(), "\"free\"");
        assert_eq!(serde_json::to_string(&AccountTier::Pro).unwrap(), "\"pro\"");
        assert_eq!(serde_json::to_string(&AccountTier::Enterprise).unwrap(), "\"enterprise\"");
    }

    #[test]
    fn test_account_status_serialization() {
        assert_eq!(serde_json::to_string(&AccountStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&AccountStatus::Suspended).unwrap(), "\"suspended\"");
        assert_eq!(serde_json::to_string(&AccountStatus::Deleted).unwrap(), "\"deleted\"");
    }

    #[test]
    fn test_channel_status_serialization() {
        assert_eq!(serde_json::to_string(&ChannelStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&ChannelStatus::Paused).unwrap(), "\"paused\"");
        assert_eq!(serde_json::to_string(&ChannelStatus::Deleted).unwrap(), "\"deleted\"");
    }

    #[test]
    fn test_signal_urgency_serialization() {
        assert_eq!(serde_json::to_string(&SignalUrgency::Low).unwrap(), "\"low\"");
        assert_eq!(serde_json::to_string(&SignalUrgency::Normal).unwrap(), "\"normal\"");
        assert_eq!(serde_json::to_string(&SignalUrgency::High).unwrap(), "\"high\"");
        assert_eq!(serde_json::to_string(&SignalUrgency::Critical).unwrap(), "\"critical\"");
    }

    #[test]
    fn test_signal_urgency_deserialization() {
        assert_eq!(serde_json::from_str::<SignalUrgency>("\"low\"").unwrap(), SignalUrgency::Low);
        assert_eq!(serde_json::from_str::<SignalUrgency>("\"normal\"").unwrap(), SignalUrgency::Normal);
        assert_eq!(serde_json::from_str::<SignalUrgency>("\"high\"").unwrap(), SignalUrgency::High);
        assert_eq!(serde_json::from_str::<SignalUrgency>("\"critical\"").unwrap(), SignalUrgency::Critical);
    }

    #[test]
    fn test_webhook_status_serialization() {
        assert_eq!(serde_json::to_string(&WebhookStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&WebhookStatus::Paused).unwrap(), "\"paused\"");
        assert_eq!(serde_json::to_string(&WebhookStatus::Disabled).unwrap(), "\"disabled\"");
    }

    #[test]
    fn test_delivery_status_serialization() {
        assert_eq!(serde_json::to_string(&DeliveryStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&DeliveryStatus::Success).unwrap(), "\"success\"");
        assert_eq!(serde_json::to_string(&DeliveryStatus::Failed).unwrap(), "\"failed\"");
    }

    #[test]
    fn test_delivery_mode_serialization() {
        assert_eq!(serde_json::to_string(&DeliveryMode::Agent).unwrap(), "\"agent\"");
        assert_eq!(serde_json::to_string(&DeliveryMode::Webhook).unwrap(), "\"webhook\"");
    }

    #[test]
    fn test_api_key_owner_serialization() {
        assert_eq!(serde_json::to_string(&ApiKeyOwner::Publisher).unwrap(), "\"publisher\"");
        assert_eq!(serde_json::to_string(&ApiKeyOwner::Subscriber).unwrap(), "\"subscriber\"");
    }

    #[test]
    fn test_api_key_status_serialization() {
        assert_eq!(serde_json::to_string(&ApiKeyStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&ApiKeyStatus::Revoked).unwrap(), "\"revoked\"");
        assert_eq!(serde_json::to_string(&ApiKeyStatus::Expired).unwrap(), "\"expired\"");
    }

    #[test]
    fn test_subscription_status_serialization() {
        assert_eq!(serde_json::to_string(&SubscriptionStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&SubscriptionStatus::Paused).unwrap(), "\"paused\"");
        assert_eq!(serde_json::to_string(&SubscriptionStatus::Canceled).unwrap(), "\"canceled\"");
    }

    #[test]
    fn test_signal_status_serialization() {
        assert_eq!(serde_json::to_string(&SignalStatus::Active).unwrap(), "\"active\"");
        assert_eq!(serde_json::to_string(&SignalStatus::Deleted).unwrap(), "\"deleted\"");
    }

    // ============================================================
    // Invalid Deserialization Tests
    // ============================================================

    #[test]
    fn test_invalid_enum_deserialization() {
        assert!(serde_json::from_str::<PricingTier>("\"invalid\"").is_err());
        assert!(serde_json::from_str::<SignalUrgency>("\"CRITICAL\"").is_err()); // case-sensitive
        assert!(serde_json::from_str::<DeliveryMode>("\"http\"").is_err());
    }

    // ============================================================
    // Struct Serialization Tests
    // ============================================================

    #[test]
    fn test_delivery_job_serialization() {
        let job = DeliveryJob {
            signal_id: "sig_123".to_string(),
            subscription_id: "sub_456".to_string(),
            webhook_id: Some("wh_789".to_string()),
            attempt: 3,
        };

        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("\"signal_id\":\"sig_123\""));
        assert!(json.contains("\"attempt\":3"));

        let parsed: DeliveryJob = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.signal_id, "sig_123");
        assert_eq!(parsed.attempt, 3);
    }

    #[test]
    fn test_delivery_job_without_webhook() {
        let job = DeliveryJob {
            signal_id: "sig_test".to_string(),
            subscription_id: "sub_test".to_string(),
            webhook_id: None,
            attempt: 1,
        };

        let json = serde_json::to_string(&job).unwrap();
        assert!(json.contains("\"webhook_id\":null"));

        let parsed: DeliveryJob = serde_json::from_str(&json).unwrap();
        assert!(parsed.webhook_id.is_none());
    }
}
