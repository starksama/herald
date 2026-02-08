# Herald - Complete Specification

> The signal marketplace for AI agents.

**Version:** 1.0  
**Date:** 2026-02-08

---

## Executive Summary

Herald connects **publishers** (businesses, news agencies, monitoring services, individuals) with **AI agents** who need real-time information. Publishers push signals, agents receive webhooks. Herald handles routing, delivery, retries, and billing.

**Core Value:** Businesses already produce announcements. Herald gives them distribution to the agent economy.

---

## Tech Stack

**Decision: Axum + sqlx + apalis**

Based on evaluation by Codex (GPT-5.2) and Gemini 3 Pro:

| Component | Choice | Why |
|-----------|--------|-----|
| **Web Framework** | Axum | Tokio-team maintained, Tower ecosystem, most popular |
| **Database** | PostgreSQL + sqlx | Compile-time query checking, raw SQL = reviewable |
| **Job Queue** | apalis (PostgreSQL-backed) | Transactional with signal data, built-in retries |
| **Cache** | Redis | Rate limiting, session cache |
| **HTTP Client** | reqwest | Standard Rust HTTP |
| **Serialization** | serde | Standard Rust |
| **Hosting** | Fly.io | Edge locations, easy scaling |

**Why not Loco?** Herald needs precise control over retry/backoff logic. Loco's abstractions could get in the way. Since Loco IS Axum underneath, we can upgrade later if needed.

---

## Product Requirements

### Publisher Types

1. **Businesses & Services** (Primary)
   - News agencies (Reuters, TechCrunch)
   - Cloud providers (AWS, Cloudflare) → incident alerts
   - Security vendors → CVEs, breach notifications
   - Sports data providers → live match events
   - Government agencies → regulatory updates

2. **Individual Curators**
   - Domain experts filtering signal from noise
   - Tech editors curating HN/Twitter

3. **Algorithmic Systems**
   - Flare (on-chain watcher) → whale movements, governance votes
   - Status aggregators → confirmed outages

4. **Hybrid**
   - AI detection + human confirmation

### Core Concepts

#### Channels
A named signal stream created by a publisher.
```json
{
  "id": "ch_abc123",
  "slug": "tech-news",
  "displayName": "Breaking Tech News",
  "publisher_id": "pub_xyz",
  "pricing_tier": "free",
  "price_cents": 0,
  "subscriber_count": 1420,
  "signal_count": 156
}
```

#### Signals
A single event pushed to a channel.
```json
{
  "id": "sig_xyz789",
  "channel_id": "ch_abc123",
  "title": "OpenAI releases GPT-5",
  "body": "New model with 10x context...",
  "urgency": "high",
  "metadata": {
    "source": "https://openai.com/blog/gpt5",
    "tags": ["ai", "llm"]
  },
  "created_at": "2026-02-08T06:30:00Z"
}
```

#### Subscriptions
Links a subscriber to a channel via a webhook.

#### Webhooks
Delivery endpoints registered by subscribers.

---

## Authentication & Authorization

### API Keys

- **Publisher keys:** `hld_pub_` + 24 char nanoid
- **Subscriber keys:** `hld_sub_` + 24 char nanoid
- Store SHA256 hash in database
- First 8-12 chars stored as prefix for identification

### Key Validation
```
Authorization: Bearer hld_pub_xxxxxxxxxxxx
```

### Rate Limiting (per tier)

| Tier | Requests/min |
|------|--------------|
| Free | 60 |
| Pro | 600 |
| Enterprise | 6000 |

Use token bucket algorithm with Redis.

### Webhook Signature (HMAC-SHA256)

When delivering webhooks, include:
```
X-Herald-Signature: sha256=<hmac>
X-Herald-Timestamp: <unix_timestamp>
X-Herald-Delivery-Id: del_xyz789
```

Signature is computed over: `{timestamp}.{json_payload}`

Subscribers must verify:
1. Timestamp is within 5 minutes of current time
2. Signature matches expected HMAC

---

## API Endpoints

### Publisher APIs

```
POST   /v1/channels              Create channel
GET    /v1/channels/:id          Get channel details
PATCH  /v1/channels/:id          Update channel
DELETE /v1/channels/:id          Delete channel (soft)

POST   /v1/channels/:id/signals  Push a signal
GET    /v1/channels/:id/signals  List signals (paginated)
GET    /v1/channels/:id/stats    Get channel stats

GET    /v1/publisher/me          Get my publisher profile
GET    /v1/publisher/api-keys    List my API keys
POST   /v1/publisher/api-keys    Create new API key
DELETE /v1/publisher/api-keys/:id Revoke API key
```

### Subscriber APIs

```
GET    /v1/channels              List available channels (marketplace)
GET    /v1/channels/:id          Get channel details

POST   /v1/subscriptions         Subscribe to channel
GET    /v1/subscriptions         List my subscriptions
DELETE /v1/subscriptions/:id     Unsubscribe

POST   /v1/webhooks              Register webhook URL
GET    /v1/webhooks              List my webhooks
PATCH  /v1/webhooks/:id          Update webhook
DELETE /v1/webhooks/:id          Delete webhook
GET    /v1/webhooks/:id/deliveries List recent deliveries

GET    /v1/subscriber/me         Get my subscriber profile
```

### Admin APIs (internal)

```
GET    /v1/admin/signals/:id     Get signal details + delivery status
GET    /v1/admin/dlq             List dead letter queue
POST   /v1/admin/dlq/:id/retry   Retry dead letter item
```

---

## Webhook Delivery

### Payload Format

```http
POST {subscriber_webhook_url}
Authorization: Bearer {subscriber_token}  # Optional, subscriber-provided
Content-Type: application/json
X-Herald-Signature: sha256={hmac}
X-Herald-Timestamp: 1707388200
X-Herald-Delivery-Id: del_xyz789

{
  "event": "signal",
  "channel": {
    "id": "ch_abc123",
    "slug": "tech-news",
    "displayName": "Breaking Tech News"
  },
  "signal": {
    "id": "sig_xyz789",
    "title": "OpenAI releases GPT-5",
    "body": "New model with 10x context...",
    "urgency": "high",
    "timestamp": "2026-02-08T10:30:00Z",
    "metadata": {}
  }
}
```

### Delivery Guarantees

- **At-least-once delivery** — use `signal.id` for deduplication
- **Timeout:** 30 seconds per attempt
- **Success:** HTTP 2xx response

### Retry Strategy (Exponential Backoff)

| Attempt | Delay | Total Elapsed |
|---------|-------|---------------|
| 1 | Immediate | 0s |
| 2 | 1 minute | 1m |
| 3 | 5 minutes | 6m |
| 4 | 30 minutes | 36m |
| 5 | 2 hours | 2h 36m |
| 6 (final) | 6 hours | 8h 36m |

After 6 failed attempts → Dead Letter Queue + alert.

### Priority Handling

Signals with `urgency: high` or `urgency: critical` get priority processing (separate queue).

---

## OpenClaw Integration

Herald is designed as a first-class signal source for OpenClaw agents.

### OpenClaw Hook Endpoints

1. `POST /hooks/wake` — Wake the main session
   ```json
   { "text": "Breaking: AWS us-east-1 down", "mode": "now" }
   ```

2. `POST /hooks/agent` — Spawn isolated agent session
   ```json
   {
     "message": "Summarize this news and decide if user should know",
     "name": "Herald",
     "sessionKey": "herald:tech-news:sig_123",
     "deliver": true,
     "channel": "telegram",
     "to": "12345678"
   }
   ```

### Security: Prompt Injection Protection

OpenClaw wraps external content safely by default:
- All external content is sandboxed
- Agent sees payload as data, not instructions
- Herald signs all payloads (HMAC-SHA256)

### Example OpenClaw Config

```json
{
  "hooks": {
    "enabled": true,
    "token": "openclaw-hook-secret",
    "mappings": [
      {
        "match": { "source": "herald" },
        "action": "agent",
        "template": {
          "message": "Herald signal: {{payload.signal.title}}. {{payload.signal.body}}",
          "name": "Herald",
          "sessionKey": "herald:{{payload.channel.slug}}:{{payload.signal.id}}",
          "deliver": true
        }
      }
    ]
  }
}
```

---

## Database Schema

### Tables

1. **publishers** — Organizations/individuals that create channels
   - id, name, email, stripe_customer_id, stripe_connect_id, tier, status

2. **subscribers** — AI agents that receive signals
   - id, name, email, webhook_secret, stripe_customer_id, tier, status

3. **channels** — Signal streams
   - id, publisher_id, slug, display_name, description, category
   - pricing_tier, price_cents, status, is_public
   - signal_count, subscriber_count (denormalized)

4. **signals** — Individual events
   - id, channel_id, title, body, urgency, metadata
   - delivery_count, delivered_count, failed_count, status

5. **subscriptions** — Links subscribers to channels
   - id, subscriber_id, channel_id, webhook_id, status
   - stripe_subscription_id (for paid channels)

6. **webhooks** — Delivery endpoints
   - id, subscriber_id, url, name, token, status
   - failure_count, last_success_at, last_failure_at

7. **deliveries** — Webhook delivery attempts (for debugging)
   - id, signal_id, subscription_id, webhook_id
   - attempt, status, status_code, error_message, latency_ms

8. **api_keys** — Authentication tokens
   - id, key_hash, key_prefix, owner_type, owner_id
   - name, scopes, last_used_at, expires_at, status

9. **dead_letter_queue** — Failed deliveries for investigation
   - id, delivery_id, signal_id, subscription_id
   - payload, error_history, resolved_at

### Key Indexes

- channels(slug) UNIQUE
- channels(publisher_id)
- channels(is_public, status) for marketplace
- signals(channel_id, created_at)
- subscriptions(channel_id) WHERE status = 'active'
- deliveries(signal_id)
- api_keys(key_hash) WHERE status = 'active'

---

## Job Queue (apalis)

Use **apalis** with PostgreSQL backend for transactional job processing.

### Job Types

1. **DeliveryJob** — Deliver a signal to a single subscriber
   ```rust
   struct DeliveryJob {
       signal_id: String,
       subscription_id: String,
       webhook_id: String,
       attempt: i32,
   }
   ```

2. **SignalStatsJob** — Update signal delivery statistics
3. **WebhookHealthJob** — Check webhook health after failures
4. **DLQAlertJob** — Alert on dead letter queue items

### Why apalis over Redis Streams?

- PostgreSQL-backed = transactional with signal data
- Built-in retry with exponential backoff
- Dead letter queue handling
- Dashboard-ready (can query job status via SQL)
- One fewer infrastructure component (no Redis for jobs)

Redis is still used for:
- Rate limiting counters
- Session cache
- Real-time metrics

---

## Project Structure

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
│   ├── worker/                # Job processor (apalis)
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

## Infrastructure

### Production Stack

| Component | Service | Tier |
|-----------|---------|------|
| Compute | Fly.io | Shared CPU |
| Database | Neon PostgreSQL | Free → Pro |
| Cache | Upstash Redis | Free → Pro |
| CDN | Cloudflare | Free |
| Monitoring | Axiom | Free |
| Payments | Stripe | Standard |

### Scaling Strategy

| Scale | API | Workers | DB | Redis |
|-------|-----|---------|-----|-------|
| MVP (100 subs) | 1 | 1 | Neon Free | Upstash Free |
| Growth (1K subs) | 2 | 3 | Neon Pro | Upstash Pro |
| Scale (10K subs) | 4 | 8 | Neon Pro | Upstash Pro |

---

## Revenue Model

Herald takes 20% of paid channel subscriptions.

| Tier | Price | Herald Cut |
|------|-------|------------|
| Free | $0 | — |
| Pro | $X/mo | 20% |
| Enterprise | Custom | 15% |

**Future:** Token-based micropayments per signal.

---

## MVP Scope (Phase 1 - 2 weeks)

### Must Have
- [ ] Publisher: create channel, push signal
- [ ] Subscriber: list channels, subscribe, register webhook
- [ ] Webhook delivery with retries (apalis)
- [ ] HMAC signature verification
- [ ] API key authentication
- [ ] Rate limiting
- [ ] Health endpoint

### Nice to Have
- [ ] Publisher dashboard (web UI)
- [ ] Channel stats endpoint
- [ ] Delivery status tracking

### Out of Scope (Phase 2+)
- Billing (Stripe)
- Channel marketplace UI
- Publisher payouts
- Analytics dashboard

---

## Security Checklist

- [ ] HTTPS only (Fly.io handles TLS)
- [ ] API keys hashed with SHA256
- [ ] Webhook signatures (HMAC-SHA256)
- [ ] Rate limiting per API key
- [ ] Webhook URL validation (HTTPS, no localhost in prod)
- [ ] Request ID logging for audit trail
- [ ] Security headers (CSP, X-Frame-Options, etc.)

---

## Observability

### Logging
- Structured JSON logs (tracing + tracing-subscriber)
- Request ID correlation
- Log to stdout → Axiom

### Metrics (Prometheus format at /metrics)
- `herald_http_requests_total{method, path, status}`
- `herald_signals_total{channel, urgency}`
- `herald_deliveries_total{status}`
- `herald_delivery_latency_seconds{channel}`
- `herald_queue_depth{queue}`

### Alerting
- Dead letter queue depth > 10
- Webhook failure rate > 10%
- API error rate > 5%
- P99 latency > 2s

---

## Related Projects

- **Flare** — On-chain event watcher, becomes a Herald publisher
- **OpenClaw** — First-class consumer of Herald signals
- **Moltbook** — Agent social network (potential distribution channel)

---

## Links

- Repo: `github.com/antoncoding/herald`
- OpenClaw Hooks: `/Users/anton/clawd/vault/research/openclaw-hooks.md`
