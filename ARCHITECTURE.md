# Herald Architecture

> Production-ready, end-to-end technical blueprint for building Herald.

**Version:** 1.0  
**Date:** 2026-02-08

---

## 1. System Overview

Herald is a signal marketplace that routes publisher signals to subscriber webhooks with at-least-once delivery, retries, and observability. The system is split into an HTTP API (Axum) and background workers (apalis) backed by PostgreSQL and Redis.

### 1.1 Architecture Diagram (ASCII)

```
                                   +-------------------------+
                                   |        OpenClaw         |
                                   |  /hooks/wake, /hooks/agent
                                   +-----------+-------------+
                                               ^
                                               |
+-------------+       HTTPS        +-----------+-------------+
| Publishers  |  --->  API  ---->  |        Herald API        |
| (API keys)  |                    |  Axum + sqlx + Redis     |
+-------------+                    +-----------+-------------+
                                              | (enqueue)
                                              v
                                       +------+------+
                                       |  apalis     |
                                       |  Job Queue  |
                                       +------+------+
                                              | (fetch)
                                              v
+-------------+       HTTPS        +-----------+-------------+
| Subscribers |  <--- Webhooks --- |      Herald Workers     |
| (HMAC)      |                    |  delivery + retries     |
+-------------+                    +-----------+-------------+
                                              |
                                              v
                                    +---------+---------+
                                    |   PostgreSQL      |
                                    | (core data + jobs)|
                                    +---------+---------+
                                              |
                                              v
                                      +-------+-------+
                                      |      Redis    |
                                      | rate limit    |
                                      +---------------+
```

### 1.2 Request Flow (High Level)

1. Publisher sends `POST /v1/channels/:id/signals` with API key.
2. API validates key, rate limit, and payload. Signal is stored in `signals`.
3. For each active subscription, a `DeliveryJob` is enqueued in apalis.
4. Worker consumes `DeliveryJob`, signs payload (HMAC-SHA256) and delivers via HTTPS.
5. Delivery attempt is recorded in `deliveries`. Success updates stats, failure schedules retry.
6. After final retry, job is placed into `dead_letter_queue` and alert job is enqueued.

---

## 2. Project Structure

Cargo workspace layout (all crates and responsibilities):

```
herald/
├── Cargo.toml                 # Workspace manifest
├── crates/
│   ├── api/                   # Axum HTTP server
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── routes/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── channels.rs
│   │   │   │   ├── signals.rs
│   │   │   │   ├── subscriptions.rs
│   │   │   │   ├── webhooks.rs
│   │   │   │   └── health.rs
│   │   │   ├── middleware/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── auth.rs
│   │   │   │   └── rate_limit.rs
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   │
│   ├── worker/                # apalis job processor
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   └── jobs/
│   │   │       ├── mod.rs
│   │   │       ├── delivery.rs
│   │   │       └── stats.rs
│   │   └── Cargo.toml
│   │
│   ├── core/                  # Shared types and logic
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types.rs       # Domain types
│   │   │   ├── auth.rs        # Key generation, HMAC signing
│   │   │   └── config.rs
│   │   └── Cargo.toml
│   │
│   └── db/                    # Database layer
│       ├── src/
│       │   ├── lib.rs
│       │   ├── models.rs      # sqlx FromRow structs
│       │   └── queries/
│       │       ├── mod.rs
│       │       ├── channels.rs
│       │       ├── signals.rs
│       │       └── ...
│       └── Cargo.toml
│
├── migrations/                # sqlx migrations
│   ├── 20260208_001_initial.sql
│   └── ...
│
├── deploy/
│   ├── Dockerfile
│   ├── fly.api.toml
│   └── fly.worker.toml
│
└── scripts/
    └── dev-setup.sh
```

---

## 3. Database Schema

### 3.1 SQL DDL (Complete)

```sql
-- enums
CREATE TYPE pricing_tier AS ENUM ('free', 'pro', 'enterprise');
CREATE TYPE account_tier AS ENUM ('free', 'pro', 'enterprise');
CREATE TYPE account_status AS ENUM ('active', 'suspended', 'deleted');
CREATE TYPE channel_status AS ENUM ('active', 'paused', 'deleted');
CREATE TYPE signal_urgency AS ENUM ('low', 'normal', 'high', 'critical');
CREATE TYPE signal_status AS ENUM ('active', 'deleted');
CREATE TYPE subscription_status AS ENUM ('active', 'paused', 'canceled');
CREATE TYPE webhook_status AS ENUM ('active', 'paused', 'disabled');
CREATE TYPE delivery_status AS ENUM ('pending', 'success', 'failed');
CREATE TYPE api_key_owner AS ENUM ('publisher', 'subscriber');
CREATE TYPE api_key_status AS ENUM ('active', 'revoked', 'expired');

-- publishers
CREATE TABLE publishers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  stripe_customer_id TEXT,
  stripe_connect_id TEXT,
  tier account_tier NOT NULL DEFAULT 'free',
  status account_status NOT NULL DEFAULT 'active',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- subscribers
CREATE TABLE subscribers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  webhook_secret TEXT NOT NULL,
  stripe_customer_id TEXT,
  tier account_tier NOT NULL DEFAULT 'free',
  status account_status NOT NULL DEFAULT 'active',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- channels
CREATE TABLE channels (
  id TEXT PRIMARY KEY,
  publisher_id TEXT NOT NULL REFERENCES publishers(id) ON DELETE CASCADE,
  slug TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  description TEXT,
  category TEXT,
  pricing_tier pricing_tier NOT NULL DEFAULT 'free',
  price_cents INTEGER NOT NULL DEFAULT 0,
  status channel_status NOT NULL DEFAULT 'active',
  is_public BOOLEAN NOT NULL DEFAULT TRUE,
  signal_count INTEGER NOT NULL DEFAULT 0,
  subscriber_count INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- signals
CREATE TABLE signals (
  id TEXT PRIMARY KEY,
  channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  urgency signal_urgency NOT NULL DEFAULT 'normal',
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  delivery_count INTEGER NOT NULL DEFAULT 0,
  delivered_count INTEGER NOT NULL DEFAULT 0,
  failed_count INTEGER NOT NULL DEFAULT 0,
  status signal_status NOT NULL DEFAULT 'active',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- webhooks
CREATE TABLE webhooks (
  id TEXT PRIMARY KEY,
  subscriber_id TEXT NOT NULL REFERENCES subscribers(id) ON DELETE CASCADE,
  url TEXT NOT NULL,
  name TEXT NOT NULL,
  token TEXT,
  status webhook_status NOT NULL DEFAULT 'active',
  failure_count INTEGER NOT NULL DEFAULT 0,
  last_success_at TIMESTAMPTZ,
  last_failure_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- subscriptions
CREATE TABLE subscriptions (
  id TEXT PRIMARY KEY,
  subscriber_id TEXT NOT NULL REFERENCES subscribers(id) ON DELETE CASCADE,
  channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
  webhook_id TEXT NOT NULL REFERENCES webhooks(id) ON DELETE RESTRICT,
  status subscription_status NOT NULL DEFAULT 'active',
  stripe_subscription_id TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (subscriber_id, channel_id)
);

-- deliveries
CREATE TABLE deliveries (
  id TEXT PRIMARY KEY,
  signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
  subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
  webhook_id TEXT NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
  attempt INTEGER NOT NULL,
  status delivery_status NOT NULL DEFAULT 'pending',
  status_code INTEGER,
  error_message TEXT,
  latency_ms INTEGER,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- api_keys
CREATE TABLE api_keys (
  id TEXT PRIMARY KEY,
  key_hash TEXT NOT NULL,
  key_prefix TEXT NOT NULL,
  owner_type api_key_owner NOT NULL,
  owner_id TEXT NOT NULL,
  name TEXT,
  scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  last_used_at TIMESTAMPTZ,
  expires_at TIMESTAMPTZ,
  status api_key_status NOT NULL DEFAULT 'active',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- dead_letter_queue
CREATE TABLE dead_letter_queue (
  id TEXT PRIMARY KEY,
  delivery_id TEXT NOT NULL REFERENCES deliveries(id) ON DELETE CASCADE,
  signal_id TEXT NOT NULL REFERENCES signals(id) ON DELETE CASCADE,
  subscription_id TEXT NOT NULL REFERENCES subscriptions(id) ON DELETE CASCADE,
  payload JSONB NOT NULL,
  error_history JSONB NOT NULL DEFAULT '[]'::jsonb,
  resolved_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- indexes
CREATE UNIQUE INDEX idx_channels_slug ON channels (slug);
CREATE INDEX idx_channels_publisher ON channels (publisher_id);
CREATE INDEX idx_channels_marketplace ON channels (is_public, status);
CREATE INDEX idx_signals_channel_time ON signals (channel_id, created_at DESC);
CREATE INDEX idx_subscriptions_channel_active ON subscriptions (channel_id)
  WHERE status = 'active';
CREATE INDEX idx_deliveries_signal ON deliveries (signal_id);
CREATE UNIQUE INDEX idx_api_keys_hash_active ON api_keys (key_hash)
  WHERE status = 'active';
```

---

## 4. API Design

All APIs use `Authorization: Bearer <api_key>` and JSON bodies. Errors use a standard format (see Section 9).

### 4.1 Publisher APIs

#### Create Channel

`POST /v1/channels`

Request:
```json
{
  "slug": "tech-news",
  "displayName": "Breaking Tech News",
  "description": "Curated tech announcements",
  "category": "technology",
  "pricingTier": "free",
  "priceCents": 0,
  "isPublic": true
}
```

Response:
```json
{
  "id": "ch_abc123",
  "slug": "tech-news",
  "displayName": "Breaking Tech News",
  "pricingTier": "free",
  "priceCents": 0,
  "subscriberCount": 0,
  "signalCount": 0
}
```

#### Get Channel Details

`GET /v1/channels/:id`

Response:
```json
{
  "id": "ch_abc123",
  "slug": "tech-news",
  "displayName": "Breaking Tech News",
  "description": "Curated tech announcements",
  "category": "technology",
  "pricingTier": "free",
  "priceCents": 0,
  "status": "active",
  "isPublic": true
}
```

#### Update Channel

`PATCH /v1/channels/:id`

Request:
```json
{
  "displayName": "Breaking Tech News (Updated)",
  "description": "Updated description"
}
```

Response:
```json
{
  "id": "ch_abc123",
  "displayName": "Breaking Tech News (Updated)",
  "updatedAt": "2026-02-08T07:10:00Z"
}
```

#### Delete Channel (Soft)

`DELETE /v1/channels/:id`

Response:
```json
{
  "id": "ch_abc123",
  "status": "deleted"
}
```

#### Push Signal

`POST /v1/channels/:id/signals`

Request:
```json
{
  "title": "OpenAI releases GPT-5",
  "body": "New model with 10x context...",
  "urgency": "high",
  "metadata": {
    "source": "https://openai.com/blog/gpt5",
    "tags": ["ai", "llm"]
  }
}
```

Response:
```json
{
  "id": "sig_xyz789",
  "channelId": "ch_abc123",
  "status": "active",
  "createdAt": "2026-02-08T06:30:00Z"
}
```

#### List Signals

`GET /v1/channels/:id/signals?limit=50&cursor=...`

Response:
```json
{
  "items": [
    {
      "id": "sig_xyz789",
      "title": "OpenAI releases GPT-5",
      "urgency": "high",
      "createdAt": "2026-02-08T06:30:00Z"
    }
  ],
  "nextCursor": "sig_000120"
}
```

#### Channel Stats

`GET /v1/channels/:id/stats`

Response:
```json
{
  "signalCount": 156,
  "subscriberCount": 1420,
  "deliverySuccessRate": 0.982
}
```

#### Get Publisher Profile

`GET /v1/publisher/me`

Response:
```json
{
  "id": "pub_001",
  "name": "Example Publisher",
  "email": "publisher@example.com",
  "tier": "pro",
  "status": "active"
}
```

#### List Publisher API Keys

`GET /v1/publisher/api-keys`

Response:
```json
{
  "items": [
    {
      "id": "key_001",
      "prefix": "hld_pub_abc12345",
      "name": "primary",
      "status": "active",
      "createdAt": "2026-02-01T10:00:00Z"
    }
  ]
}
```

#### Create Publisher API Key

`POST /v1/publisher/api-keys`

Request:
```json
{ "name": "primary" }
```

Response:
```json
{
  "id": "key_002",
  "key": "hld_pub_xxxxxxxxxxxxxxxxxxxxxxxx",
  "prefix": "hld_pub_xxxxxxxx"
}
```

#### Revoke Publisher API Key

`DELETE /v1/publisher/api-keys/:id`

Response:
```json
{ "status": "revoked" }
```

### 4.2 Subscriber APIs

#### List Marketplace Channels

`GET /v1/channels`

Response:
```json
{
  "items": [
    {
      "id": "ch_abc123",
      "slug": "tech-news",
      "displayName": "Breaking Tech News",
      "pricingTier": "free",
      "priceCents": 0
    }
  ]
}
```

#### Get Channel Details (Marketplace)

`GET /v1/channels/:id`

Response:
```json
{
  "id": "ch_abc123",
  "slug": "tech-news",
  "displayName": "Breaking Tech News",
  "description": "Curated tech announcements",
  "pricingTier": "free",
  "priceCents": 0
}
```

#### Subscribe

`POST /v1/subscriptions`

Request:
```json
{
  "channelId": "ch_abc123",
  "webhookId": "wh_001"
}
```

Response:
```json
{
  "id": "sub_001",
  "status": "active"
}
```

#### List Subscriptions

`GET /v1/subscriptions`

Response:
```json
{
  "items": [
    {
      "id": "sub_001",
      "channelId": "ch_abc123",
      "webhookId": "wh_001",
      "status": "active"
    }
  ]
}
```

#### Unsubscribe

`DELETE /v1/subscriptions/:id`

Response:
```json
{
  "id": "sub_001",
  "status": "canceled"
}
```

#### Register Webhook

`POST /v1/webhooks`

Request:
```json
{
  "name": "Primary",
  "url": "https://example.com/webhooks/herald",
  "token": "subscriber-token-optional"
}
```

Response:
```json
{
  "id": "wh_001",
  "status": "active"
}
```

#### List Webhooks

`GET /v1/webhooks`

Response:
```json
{
  "items": [
    {
      "id": "wh_001",
      "name": "Primary",
      "url": "https://example.com/webhooks/herald",
      "status": "active"
    }
  ]
}
```

#### Update Webhook

`PATCH /v1/webhooks/:id`

Request:
```json
{
  "name": "Primary (Updated)",
  "url": "https://example.com/webhooks/herald"
}
```

Response:
```json
{
  "id": "wh_001",
  "status": "active",
  "updatedAt": "2026-02-08T07:20:00Z"
}
```

#### Delete Webhook

`DELETE /v1/webhooks/:id`

Response:
```json
{
  "id": "wh_001",
  "status": "disabled"
}
```

#### List Webhook Deliveries

`GET /v1/webhooks/:id/deliveries?limit=50&cursor=...`

Response:
```json
{
  "items": [
    {
      "id": "del_001",
      "status": "success",
      "attempt": 1,
      "statusCode": 200,
      "latencyMs": 120
    }
  ],
  "nextCursor": "del_000080"
}
```

#### Get Subscriber Profile

`GET /v1/subscriber/me`

Response:
```json
{
  "id": "sub_001",
  "name": "Agent Alpha",
  "email": "agent@example.com",
  "tier": "free",
  "status": "active"
}
```

### 4.3 Admin APIs (Internal)

#### DLQ List

`GET /v1/admin/dlq`

Response:
```json
{
  "items": [
    {
      "id": "dlq_001",
      "signalId": "sig_xyz789",
      "subscriptionId": "sub_001",
      "createdAt": "2026-02-08T08:30:00Z"
    }
  ]
}
```

#### DLQ Retry

`POST /v1/admin/dlq/:id/retry`

Response:
```json
{ "status": "queued" }
```

#### Get Signal Details + Delivery Status

`GET /v1/admin/signals/:id`

Response:
```json
{
  "signal": {
    "id": "sig_xyz789",
    "title": "OpenAI releases GPT-5",
    "urgency": "high",
    "createdAt": "2026-02-08T06:30:00Z"
  },
  "deliveries": [
    {
      "id": "del_001",
      "status": "success",
      "attempt": 1,
      "statusCode": 200
    }
  ]
}
```

---

## 5. Job Queue (apalis)

### 5.1 apalis Setup

- Use PostgreSQL backend for transactional job processing.
- Separate queues for high urgency vs normal.
- Built-in retries with exponential backoff.

Example setup (worker):

```rust
use apalis::{prelude::*, layers::RetryLayer};
use apalis::postgres::PostgresStorage;
use std::time::Duration;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DeliveryJob {
    pub signal_id: String,
    pub subscription_id: String,
    pub webhook_id: String,
    pub attempt: i32,
}

pub fn retry_policy(attempt: u32) -> Duration {
    match attempt {
        0 => Duration::from_secs(0),
        1 => Duration::from_secs(60),
        2 => Duration::from_secs(300),
        3 => Duration::from_secs(1800),
        4 => Duration::from_secs(7200),
        _ => Duration::from_secs(21600),
    }
}

pub async fn run_worker(db_url: &str) -> anyhow::Result<()> {
    let storage = PostgresStorage::new(db_url).await?;

    let worker = WorkerBuilder::new("delivery")
        .layer(RetryLayer::new(retry_policy))
        .build_fn(|job: DeliveryJob| async move {
            // deliver signal here
            Ok(())
        });

    Monitor::new()
        .register(worker)
        .run()
        .await?;

    Ok(())
}
```

### 5.2 Job Types

- `DeliveryJob` — deliver a signal to a subscriber
- `SignalStatsJob` — update delivery stats
- `WebhookHealthJob` — background health check after failures
- `DLQAlertJob` — notify when DLQ grows

### 5.3 Enqueue Strategy (API)
Use separate queues for priority handling. Urgent signals go to `delivery-high`, all others to `delivery-normal`.

```rust
use apalis::prelude::*;
use apalis::postgres::PostgresStorage;

pub async fn enqueue_deliveries(
    storage: &PostgresStorage,
    job: DeliveryJob,
    urgency: &str,
) -> anyhow::Result<()> {
    let queue = if matches!(urgency, "high" | "critical") {
        "delivery-high"
    } else {
        "delivery-normal"
    };

    storage.push(queue, job).await?;
    Ok(())
}
```

---

## 6. Authentication

### 6.1 API Key Management

- Publisher keys: `hld_pub_` + 24-char nanoid
- Subscriber keys: `hld_sub_` + 24-char nanoid
- Store SHA256 hash in database
- Store first 8–12 chars as `key_prefix`

Key generation (core):

```rust
use nanoid::nanoid;
use sha2::{Sha256, Digest};

pub fn generate_api_key(prefix: &str) -> (String, String, String) {
    let raw = format!("{}{}", prefix, nanoid!(24));
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let key_prefix = raw.chars().take(12).collect::<String>();
    (raw, hash, key_prefix)
}
```

### 6.2 HMAC Signing

Signature is computed over `{timestamp}.{json_payload}`.

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn sign_payload(secret: &str, timestamp: i64, body: &str) -> String {
    let data = format!("{}.{}", timestamp, body);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC key");
    mac.update(data.as_bytes());
    format!("sha256={:x}", mac.finalize().into_bytes())
}
```

### 6.3 API Key Validation Middleware (Axum)

```rust
use axum::{async_trait, http::Request, middleware::Next, response::Response};
use axum::http::StatusCode;
use sha2::{Sha256, Digest};

pub async fn require_api_key<B>(mut req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    let auth = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if token.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    // Query api_keys by hash. If found, attach owner info to request extensions.
    // If not found or revoked, reject.
    // Example assumes a function `lookup_api_key(&hash)`.

    req.extensions_mut().insert(hash);
    Ok(next.run(req).await)
}
```

---

## 7. Rate Limiting

Redis token bucket per API key, per minute based on tier.

```rust
use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn allow_request(
    mut redis: redis::aio::Connection,
    key: &str,
    capacity: u32,
    refill_per_min: u32,
) -> redis::RedisResult<bool> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let bucket_key = format!("rl:{}", key);

    let script = r#"
local bucket = KEYS[1]
local now = tonumber(ARGV[1])
local capacity = tonumber(ARGV[2])
local refill = tonumber(ARGV[3])

local data = redis.call('HMGET', bucket, 'tokens', 'ts')
local tokens = tonumber(data[1]) or capacity
local ts = tonumber(data[2]) or now

local delta = math.max(0, now - ts)
local new_tokens = math.min(capacity, tokens + (delta * refill / 60))

if new_tokens < 1 then
  redis.call('HMSET', bucket, 'tokens', new_tokens, 'ts', now)
  redis.call('EXPIRE', bucket, 120)
  return 0
else
  new_tokens = new_tokens - 1
  redis.call('HMSET', bucket, 'tokens', new_tokens, 'ts', now)
  redis.call('EXPIRE', bucket, 120)
  return 1
end
"#;

    let allowed: i32 = redis::Script::new(script)
        .key(bucket_key)
        .arg(now)
        .arg(capacity)
        .arg(refill_per_min)
        .invoke_async(&mut redis)
        .await?;

    Ok(allowed == 1)
}
```

Tier defaults:

- `free`: 60 requests/min
- `pro`: 600 requests/min
- `enterprise`: 6000 requests/min

---

## 8. Webhook Delivery

### 8.1 Delivery Flow

1. Worker loads `signal`, `subscription`, `webhook`.
2. Constructs payload with channel and signal info.
3. Adds HMAC headers: `X-Herald-Signature`, `X-Herald-Timestamp`, `X-Herald-Delivery-Id`.
4. POSTs to webhook URL with 30s timeout.
5. Writes to `deliveries`. Updates counters.
6. On failure, schedules retry based on attempt.

### 8.2 Signature Verification (Subscriber Side)

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn verify_signature(secret: &str, timestamp: i64, body: &str, signature: &str) -> bool {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    if (now - timestamp).abs() > 300 {
        return false;
    }

    let data = format!("{}.{}", timestamp, body);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(data.as_bytes());
    let expected = format!("sha256={:x}", mac.finalize().into_bytes());

    subtle::ConstantTimeEq::ct_eq(expected.as_bytes(), signature.as_bytes()).into()
}
```

### 8.3 Delivery Worker (Rust Example)

```rust
use reqwest::Client;
use serde_json::json;
use std::time::Instant;

pub async fn deliver_signal(
    client: &Client,
    webhook_url: &str,
    webhook_token: Option<&str>,
    hmac_secret: &str,
    delivery_id: &str,
    payload: serde_json::Value,
) -> anyhow::Result<(u16, i64)> {
    let body = serde_json::to_string(&payload)?;
    let timestamp = chrono::Utc::now().timestamp();
    let signature = crate::auth::sign_payload(hmac_secret, timestamp, &body);

    let mut req = client
        .post(webhook_url)
        .header("Content-Type", "application/json")
        .header("X-Herald-Signature", signature)
        .header("X-Herald-Timestamp", timestamp.to_string())
        .header("X-Herald-Delivery-Id", delivery_id);

    if let Some(token) = webhook_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let start = Instant::now();
    let resp = req.body(body).send().await?;
    let latency_ms = start.elapsed().as_millis() as i64;

    Ok((resp.status().as_u16(), latency_ms))
}
```

### 8.4 OpenClaw Hooks (Optional)

Herald can forward signals to OpenClaw as a subscriber. Use `/hooks/wake` for immediate notifications or `/hooks/agent` to spawn agent sessions.

- `POST /hooks/wake`
- `POST /hooks/agent`

---

## 9. Error Handling

### 9.1 Error Type

```rust
use axum::{http::StatusCode, Json, response::IntoResponse};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub request_id: String,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest(&'static str),
    Unauthorized,
    NotFound,
    RateLimited,
    Internal,
}

impl AppError {
    pub fn into_response(self, request_id: &str) -> impl IntoResponse {
        let (status, code, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "invalid_request", msg),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "Invalid API key"),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", "Resource not found"),
            AppError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited", "Too many requests"),
            AppError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Unexpected error"),
        };

        (
            status,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: code.to_string(),
                    message: message.to_string(),
                    request_id: request_id.to_string(),
                },
            }),
        )
    }
}
```

### 9.2 Response Format

```json
{
  "error": {
    "code": "invalid_request",
    "message": "Missing required field: title",
    "request_id": "req_abc123"
  }
}
```

---

## 10. Configuration

### 10.1 Environment Variables

- `DATABASE_URL`
- `REDIS_URL`
- `RUST_LOG`
- `HERALD_ENV` (dev|prod)
- `HERALD_API_BIND` (e.g., 0.0.0.0:8080)
- `HERALD_WORKER_CONCURRENCY`
- `HERALD_HMAC_SECRET`
- `HERALD_RATE_LIMIT_FREE`
- `HERALD_RATE_LIMIT_PRO`
- `HERALD_RATE_LIMIT_ENT`

### 10.2 Config Struct

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub database_url: String,
    pub redis_url: String,
    pub herald_env: String,
    pub api_bind: String,
    pub worker_concurrency: usize,
    pub hmac_secret: String,
    pub rate_limit_free: u32,
    pub rate_limit_pro: u32,
    pub rate_limit_ent: u32,
}
```

---

## 11. Observability

### 11.1 Logging

- `tracing` + `tracing-subscriber` JSON output
- Correlate by request ID
- Log to stdout for Fly.io and Axiom

```rust
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_tracing() {
    let filter = EnvFilter::from_default_env();
    fmt()
        .with_env_filter(filter)
        .json()
        .init();
}
```

Request ID middleware example:

```rust
use axum::{http::Request, middleware::Next, response::Response};
use uuid::Uuid;

pub async fn request_id<B>(mut req: Request<B>, next: Next<B>) -> Response {
    let request_id = Uuid::new_v4().to_string();
    req.extensions_mut().insert(request_id.clone());
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        "X-Request-Id",
        request_id.parse().unwrap(),
    );
    resp
}
```

### 11.2 Metrics

Expose `/metrics` in Prometheus format.

Metrics to include:
- `herald_http_requests_total{method, path, status}`
- `herald_signals_total{channel, urgency}`
- `herald_deliveries_total{status}`
- `herald_delivery_latency_seconds{channel}`
- `herald_queue_depth{queue}`

---

## 12. Deployment

### 12.1 Fly.io

`deploy/fly.api.toml` and `deploy/fly.worker.toml` split API and worker apps.

Example `fly.api.toml`:

```toml
app = "herald-api"
primary_region = "iad"

[env]
HERALD_API_BIND = "0.0.0.0:8080"

[http_service]
internal_port = 8080
force_https = true
auto_start_machines = true
auto_stop_machines = true
min_machines_running = 1
```

Example `fly.worker.toml`:

```toml
app = "herald-worker"
primary_region = "iad"

[env]
HERALD_WORKER_CONCURRENCY = "8"
```

### 12.2 Dockerfile

Multi-stage build:

```Dockerfile
FROM rust:1.76 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p api -p worker

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/api /usr/local/bin/herald-api
COPY --from=builder /app/target/release/worker /usr/local/bin/herald-worker
CMD ["/usr/local/bin/herald-api"]
```

### 12.3 CI/CD

- Build + test on push
- Run `sqlx prepare` in CI
- Deploy to Fly.io on main branch

---

## 13. Development

### 13.1 Local Setup

1. Start Postgres + Redis (docker-compose or local)
2. Run migrations: `sqlx migrate run`
3. Start API: `cargo run -p api`
4. Start worker: `cargo run -p worker`

### 13.2 Testing Strategy

- Unit tests for core (auth, rate limit logic, signing)
- Integration tests for API endpoints
- Worker tests for retry + DLQ logic
- Load test webhook delivery

---

## Appendix: Request Validation & Security

- Validate webhook URLs: https only, no localhost in prod
- Enforce max payload size (e.g., 1MB)
- Request ID propagation in middleware
- Security headers on all responses
