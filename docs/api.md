# Herald API Reference

Base URL: `https://api.herald.dev/v1` (or your self-hosted instance)

Authentication: `Authorization: Bearer <api_key>`

## Authentication

API keys are prefixed by role:
- Publishers: `hld_pub_...`
- Subscribers: `hld_sub_...`

All requests must include the `Authorization` header with a valid API key.

---

## Publisher APIs

### Register Publisher

`POST /v1/publishers`

Creates a new publisher account. Returns the publisher profile and initial API key.

**Request:**
```json
{
  "name": "Acme News",
  "email": "news@acme.com"
}
```

**Response:**
```json
{
  "id": "pub_abc123",
  "name": "Acme News",
  "email": "news@acme.com",
  "tier": "free",
  "status": "active",
  "apiKey": "hld_pub_xxxx..."
}
```

### Get Publisher Profile

`GET /v1/publishers/me`

Returns the current publisher's profile.

### Create Channel

`POST /v1/channels`

**Request:**
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

**Response:**
```json
{
  "id": "ch_abc123",
  "slug": "tech-news",
  "displayName": "Breaking Tech News",
  "pricingTier": "free",
  "subscriberCount": 0,
  "signalCount": 0
}
```

### Get Channel

`GET /v1/channels/:id`

Returns channel details. Publishers see full details; subscribers see public info.

### Update Channel

`PATCH /v1/channels/:id`

**Request:**
```json
{
  "displayName": "Breaking Tech News (Updated)",
  "description": "Updated description"
}
```

### Delete Channel

`DELETE /v1/channels/:id`

Soft deletes the channel. Existing subscriptions are canceled.

### Push Signal

`POST /v1/channels/:id/signals`

Pushes a new signal to all active subscribers.

**Request:**
```json
{
  "title": "OpenAI releases GPT-5",
  "body": "New model with 10x context window, native tool use...",
  "urgency": "high",
  "metadata": {
    "source": "https://openai.com/blog/gpt5",
    "tags": ["ai", "llm", "openai"]
  }
}
```

**Response:**
```json
{
  "id": "sig_xyz789",
  "channelId": "ch_abc123",
  "status": "active",
  "createdAt": "2026-02-08T06:30:00Z"
}
```

**Urgency levels:** `low`, `normal`, `high`, `critical`

High/critical signals are delivered with higher priority.

### List Signals

`GET /v1/channels/:id/signals?limit=50&cursor=...`

Returns paginated signals for a channel.

### Channel Stats

`GET /v1/channels/:id/stats`

```json
{
  "signalCount": 156,
  "subscriberCount": 1420,
  "deliverySuccessRate": 0.982
}
```

---

## Subscriber APIs

### Register Subscriber

`POST /v1/subscribers`

**Request:**
```json
{
  "name": "Agent Alpha",
  "email": "agent@example.com"
}
```

**Response:**
```json
{
  "id": "sub_001",
  "name": "Agent Alpha",
  "email": "agent@example.com",
  "webhookSecret": "whsec_xxx...",
  "apiKey": "hld_sub_xxxx..."
}
```

The `webhookSecret` is used to verify webhook signatures.

### Get Subscriber Profile

`GET /v1/subscribers/me`

### List Channels (Marketplace)

`GET /v1/channels?category=technology&limit=20`

Returns public channels available for subscription.

### Register Webhook

`POST /v1/webhooks`

**Request:**
```json
{
  "name": "Primary",
  "url": "https://my-agent.com/webhooks/herald",
  "token": "optional-bearer-token"
}
```

**Response:**
```json
{
  "id": "wh_001",
  "name": "Primary",
  "url": "https://my-agent.com/webhooks/herald",
  "status": "active"
}
```

**Notes:**
- URLs must use HTTPS
- Optional `token` is sent as `Authorization: Bearer <token>` on delivery
- One webhook can receive signals from multiple channels (via subscriptions)

### List Webhooks

`GET /v1/webhooks`

### Update Webhook

`PATCH /v1/webhooks/:id`

### Delete Webhook

`DELETE /v1/webhooks/:id`

Webhooks with active subscriptions cannot be deleted.

### Subscribe to Channel

`POST /v1/subscriptions`

**Request:**
```json
{
  "channelId": "ch_abc123",
  "webhookId": "wh_001"
}
```

**Response:**
```json
{
  "id": "sub_001",
  "channelId": "ch_abc123",
  "webhookId": "wh_001",
  "status": "active"
}
```

**Multi-channel support:** You can subscribe the same webhook to multiple channels. Each signal delivery includes `channel_id` so you know which channel triggered it.

### List Subscriptions

`GET /v1/subscriptions`

### Unsubscribe

`DELETE /v1/subscriptions/:id`

### Webhook Deliveries

`GET /v1/webhooks/:id/deliveries?limit=50`

View delivery history for a webhook.

---

## Webhook Delivery Format

When a signal is pushed, Herald delivers to each subscriber's webhook:

```http
POST https://your-webhook.com/herald
Content-Type: application/json
Authorization: Bearer <your-token>
X-Herald-Signature: sha256=abc123...
X-Herald-Timestamp: 1707379800
X-Herald-Delivery-Id: del_xyz789

{
  "event": "signal",
  "channel_id": "ch_abc123",
  "channel_slug": "tech-news",
  "signal": {
    "id": "sig_xyz789",
    "title": "OpenAI releases GPT-5",
    "body": "New model with 10x context...",
    "urgency": "high",
    "metadata": {
      "source": "https://openai.com/blog/gpt5",
      "tags": ["ai", "llm"]
    },
    "created_at": "2026-02-08T06:30:00Z"
  }
}
```

### Signature Verification

Verify the webhook came from Herald using HMAC-SHA256:

```python
import hmac
import hashlib
import time

def verify_herald_signature(payload: str, signature: str, timestamp: str, secret: str) -> bool:
    # Check timestamp is within 5 minutes
    now = int(time.time())
    if abs(now - int(timestamp)) > 300:
        return False
    
    # Compute expected signature
    data = f"{timestamp}.{payload}"
    expected = "sha256=" + hmac.new(
        secret.encode(),
        data.encode(),
        hashlib.sha256
    ).hexdigest()
    
    return hmac.compare_digest(expected, signature)
```

### Delivery Retries

Failed deliveries are retried with exponential backoff:

| Attempt | Delay |
|---------|-------|
| 1 | Immediate |
| 2 | 1 minute |
| 3 | 5 minutes |
| 4 | 30 minutes |
| 5 | 2 hours |
| 6 | 6 hours |

After 6 attempts, the delivery goes to dead letter queue.

### Idempotency

Use `signal.id` for deduplication. Signals may be delivered multiple times (at-least-once).

---

## Rate Limits

Limits are per API key, per minute:

| Tier | Requests/min |
|------|-------------|
| Free | 60 |
| Pro | 600 |
| Enterprise | 6000 |

Rate limit headers:
```
X-RateLimit-Limit: 60
X-RateLimit-Remaining: 45
X-RateLimit-Reset: 1707379860
```

---

## Errors

All errors follow this format:

```json
{
  "error": {
    "code": "invalid_request",
    "message": "Missing required field: title",
    "request_id": "req_abc123"
  }
}
```

| Code | Status | Description |
|------|--------|-------------|
| `invalid_request` | 400 | Bad request body |
| `unauthorized` | 401 | Invalid/missing API key |
| `forbidden` | 403 | Access denied |
| `not_found` | 404 | Resource not found |
| `rate_limited` | 429 | Too many requests |
| `internal_error` | 500 | Server error |

---

## Health Check

`GET /health`

```json
{
  "status": "ok",
  "version": "1.0.0"
}
```
