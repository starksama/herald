# Herald

**The signal marketplace for AI agents.**

Herald connects publishers — businesses, news agencies, monitoring services, and individuals — with AI agents who need real-time information. Any organization that already produces valuable announcements can now sell directly to the agent economy.

---

## Quick Start

```bash
# Clone and enter
git clone https://github.com/starksama/herald.git
cd herald

# Copy env file
cp .env.example .env

# Start PostgreSQL + Redis
docker compose up -d

# Run migrations (first time only)
# Migrations run automatically via docker-entrypoint-initdb.d

# Run the API server
cargo run -p herald-api

# In another terminal, run the worker
cargo run -p herald-worker
```

The API will be available at `http://localhost:8080`.

### Docker Compose Services
- **PostgreSQL** (port 5432): `herald:herald@localhost:5432/herald`
- **Redis** (port 6379): `redis://localhost:6379`

### Useful Commands
```bash
# View logs
docker compose logs -f

# Stop services
docker compose down

# Reset everything (delete volumes)
docker compose down -v
```

---

## The Vision

**Kickstart the official agent economy.**

Today, agents are isolated. They poll APIs, scrape websites, and build custom integrations. This doesn't scale.

Herald flips the model: **publishers push to agents, not the other way around.**

### Why This Matters

Every business, news agency, and monitoring service already produces announcements. They have:
- Press releases
- Status updates
- Breaking news
- Incident reports
- Market alerts

**Today:** They publish to websites, RSS feeds, Twitter. Agents scrape unreliably.

**With Herald:** They push once, reach every subscribed agent instantly. And they get paid.

### The Opportunity

| Publisher Type | What They Sell | Agent Value |
|---------------|----------------|-------------|
| **News agencies** (Reuters, AP, TechCrunch) | Breaking news | Stay informed without scraping |
| **Cloud providers** (AWS, GCP, Cloudflare) | Incident alerts | React to outages instantly |
| **Security firms** (CrowdStrike, Snyk) | Vulnerability disclosures | Patch before exploits |
| **Sports data** (ESPN, Opta) | Live match events | Real-time updates |
| **Financial services** | Market alerts | Trading signals |
| **Individual curators** | Niche expertise | Domain-specific signal |

Herald is the **distribution layer** for anyone with valuable signals to sell.

---

## The Problem

AI agents need real-time information that:
- **Can't be cron'd** — timing is unpredictable (breaking news, incidents, live events)
- **Requires judgment** — raw data is noisy, someone filters signal from noise
- **Crosses domains** — agents shouldn't maintain 100 integrations

Today, every agent builds their own scrapers, watchers, and pollers. It's wasteful and unreliable.

---

## The Solution

Herald is a relay layer between **publishers** and **subscribers** (AI agents).

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Publishers    │     │     Herald      │     │   Subscribers   │
│ (any source)    │ ──▶ │   (relay hub)   │ ──▶ │   (AI agents)   │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                       │
   Create channels         Route signals          Receive webhooks
   Push updates           Handle auth/billing     Act on signals
   Set pricing            Track delivery          Manage subs
```

---

## Publisher Types

### 1. Businesses & Services (The big opportunity)
Organizations that already produce announcements can monetize them for agents.

**Examples:**
- **News agencies** — Reuters, Bloomberg, TechCrunch push breaking stories
- **Cloud providers** — AWS, Cloudflare push incident alerts
- **Security vendors** — Push CVEs, breach notifications
- **Sports data providers** — Push live match events
- **Government agencies** — Push regulatory updates

**Value:** They already have the signal. Herald gives them the distribution channel to agents.

### 2. Individual Curators
Humans with domain expertise who filter and curate.

**Examples:**
- Tech editor curating HN/Twitter → pushes what matters
- Crypto analyst watching chains → pushes significant movements
- Sports fan watching matches → pushes goals, red cards

**Value:** Judgment and context that can't be automated.

### 3. Algorithmic Systems
Automated systems that detect definable conditions.

**Examples:**
- Flare (on-chain watcher) → whale movements, governance votes
- Status aggregators → confirmed outages across services
- Price feeds → threshold crossings

**Value:** Speed and reliability for well-defined triggers.

### 4. Hybrid
Algorithmic detection with human oversight.

**Examples:**
- AI flags potential news → human confirms and pushes
- Bot detects anomaly → human adds context

---

## OpenClaw Integration

Herald is designed as a first-class signal source for OpenClaw agents.

### How OpenClaw Hooks Work

From the [OpenClaw docs](https://docs.openclaw.ai/automation/webhook):

**Enable hooks in config:**
```json
{
  "hooks": {
    "enabled": true,
    "token": "shared-secret",
    "path": "/hooks"
  }
}
```

**Two endpoint types:**

1. `POST /hooks/wake` — Wake the main session with a system event
   ```json
   { "text": "Breaking: AWS us-east-1 down", "mode": "now" }
   ```

2. `POST /hooks/agent` — Spawn an isolated agent session
   ```json
   {
     "message": "Summarize this breaking news and decide if user should know",
     "name": "Herald",
     "sessionKey": "herald:tech-news:evt-123",
     "deliver": true,
     "channel": "telegram",
     "to": "12345678"
   }
   ```

**Auth options:**
- `Authorization: Bearer <token>` (recommended)
- `x-openclaw-token: <token>`

### Security: Prompt Injection Protection

OpenClaw handles malicious content in webhook payloads:

> "Hook payloads are treated as untrusted and wrapped with safety boundaries by default."

- All external content is sandboxed
- Agent sees payload as data, not instructions
- `allowUnsafeExternalContent: true` can disable this (dangerous, only for trusted sources)

**Herald's approach:**
- We sign all payloads (HMAC-SHA256)
- Agents verify signature before processing
- Content is always treated as data
- No executable code in signals

### Herald → OpenClaw Flow

```
Herald Signal Fires
        │
        ▼
POST to agent's registered webhook
        │
        ▼
OpenClaw Gateway receives
        │
        ├── Verifies Herald signature
        ├── Wraps content safely
        └── Spawns isolated session
                │
                ▼
        Agent processes signal
                │
                ├── Take action (notify user, run task)
                └── Log and continue
```

### OpenClaw Config Example

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
          "message": "Herald signal received: {{payload.title}}. Channel: {{payload.channel}}. Content: {{payload.body}}",
          "name": "Herald",
          "sessionKey": "herald:{{payload.channel}}:{{payload.id}}",
          "deliver": true
        }
      }
    ]
  },
  "herald": {
    "apiKey": "hld_subscriber_xxx",
    "subscriptions": ["tech-news", "aws-outages"],
    "webhookUrl": "https://your-gateway.ts.net/hooks/herald"
  }
}
```

---

## Core Concepts

### Channels
A **channel** is a named signal stream created by a publisher.

```json
{
  "id": "ch_abc123",
  "name": "tech-news",
  "displayName": "Breaking Tech News",
  "description": "Human-curated AI/tech announcements",
  "publisher": "pub_xyz",
  "pricing": {
    "tier": "free"
  },
  "stats": {
    "subscribers": 1420,
    "signalsLast24h": 12,
    "avgDeliveryMs": 230
  }
}
```

**Example channels:**
- `tech-news` — Breaking AI/tech announcements
- `aws-outages` — Human-verified AWS incidents
- `eth-whales` — Large ETH movements (Flare integration)
- `liverpool-fc` — Live match events

### Publishers
Humans or systems that create and maintain channels. They:
- Define channel topic and signal criteria
- Push signals when events occur
- Set pricing (free, paid, tiered)
- Build reputation through quality and reliability

### Subscribers
AI agents that receive signals. They:
- Browse and subscribe to channels
- Receive webhooks when signals fire
- Manage subscriptions in one place
- Pay for premium channels (via their humans)

### Signals
A single event pushed to a channel.

```json
{
  "id": "sig_xyz789",
  "channel": "tech-news",
  "title": "OpenAI releases GPT-5",
  "body": "New model with 10x context, native tool use, significantly improved reasoning...",
  "urgency": "high",
  "timestamp": "2026-02-08T06:30:00Z",
  "signature": "sha256=...",
  "metadata": {
    "source": "https://openai.com/blog/gpt5",
    "tags": ["ai", "llm", "openai"],
    "publisher_note": "Major release, affects all AI agents"
  }
}
```

---

## Why Human-Curated?

Raw data feeds exist. What's missing is **judgment**.

| Raw Feed | Herald Channel |
|----------|----------------|
| Every HN post | "This one matters" |
| All AWS status updates | "This is actually breaking things" |
| Every on-chain tx | "This whale movement is significant" |
| Every tweet | "This is breaking news, not noise" |

Humans provide the filter. Agents get signal, not noise.

---

## Technical Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────┐
│                         Herald                                │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
│  │ Publisher   │    │   Router    │    │ Subscriber  │       │
│  │ Service     │───▶│   (Fan-out) │───▶│ Service     │       │
│  └─────────────┘    └─────────────┘    └─────────────┘       │
│         │                  │                   │              │
│         ▼                  ▼                   ▼              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
│  │ Auth        │    │ Delivery    │    │ Billing     │       │
│  │ Service     │    │ Queue       │    │ Service     │       │
│  └─────────────┘    └─────────────┘    └─────────────┘       │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

### Tech Stack (Proposed)

- **API:** Node.js + Hono (lightweight, fast)
- **Database:** PostgreSQL (channels, subscriptions, delivery logs)
- **Queue:** Redis + BullMQ (reliable delivery with retries)
- **Auth:** API keys + HMAC signatures
- **Hosting:** Fly.io or Railway (easy scaling)

### API Endpoints

#### Publisher APIs

```
POST   /v1/channels              Create channel
GET    /v1/channels/:id          Get channel details
PATCH  /v1/channels/:id          Update channel
DELETE /v1/channels/:id          Delete channel

POST   /v1/channels/:id/signals  Push a signal
GET    /v1/channels/:id/stats    Get channel stats
```

#### Subscriber APIs

```
GET    /v1/channels              List available channels
POST   /v1/subscriptions         Subscribe to channel
GET    /v1/subscriptions         List my subscriptions
DELETE /v1/subscriptions/:id     Unsubscribe

POST   /v1/webhooks              Register webhook URL
GET    /v1/webhooks              List my webhooks
PATCH  /v1/webhooks/:id          Update webhook
DELETE /v1/webhooks/:id          Delete webhook
```

#### Webhook Delivery

```
POST {subscriber_webhook_url}
Authorization: Bearer {subscriber_token}
X-Herald-Signature: sha256={hmac}
X-Herald-Timestamp: 1707379800
Content-Type: application/json

{
  "event": "signal",
  "channel": "tech-news",
  "signal": {
    "id": "sig_xyz789",
    "title": "...",
    "body": "...",
    ...
  }
}
```

### Delivery Guarantees

- **At-least-once delivery** — signals may be delivered multiple times
- **Idempotency** — use `signal.id` for deduplication
- **Retry policy:** 3 attempts with exponential backoff (1s, 5s, 30s)
- **Dead letter queue** — failed deliveries logged for debugging
- **Delivery receipts** — track success/failure per subscriber

### Authentication

#### Publishers
- API key: `hld_pub_xxx`
- Rate limits based on tier
- Webhook for delivery status (optional)

#### Subscribers
- API key: `hld_sub_xxx`
- Webhook URL + verification
- Signature verification (HMAC-SHA256)

**Signature verification example:**
```javascript
const crypto = require('crypto');

function verifySignature(payload, signature, secret) {
  const expected = crypto
    .createHmac('sha256', secret)
    .update(JSON.stringify(payload))
    .digest('hex');
  return `sha256=${expected}` === signature;
}
```

---

## Integration with Flare

Flare (on-chain event watcher) becomes a **publisher** on Herald:

```
Flare watches blockchain
        │
        ▼
Condition triggered (whale tx, governance vote, etc.)
        │
        ▼
Flare pushes to Herald channel
        │
        ▼
Herald fans out to all subscribers
        │
        ▼
Non-crypto agents receive signal too
```

This makes Flare's on-chain signals available to the broader agent ecosystem.

---

## Revenue Model

Herald takes a cut of paid channel subscriptions.

| Tier | Price | Herald Cut |
|------|-------|------------|
| Free | $0 | — |
| Pro | $X/mo | 20% |
| Enterprise | Custom | 15% |

Publishers set their own pricing. Herald handles billing and payouts.

**Future:** Token-based micropayments per signal (pay-per-use model).

---

## Demo: Agent News Wire

First showcase channel: **Breaking AI/Tech News**

### Setup
1. Human editors monitor HN, Twitter/X, tech blogs
2. When something significant drops, push to Herald
3. Subscribed agents receive instant notification

### Demo Flow
```
Editor spots GPT-5 announcement
        │
        ▼
Push signal to Herald "tech-news" channel
        │
        ▼
Herald routes to 50+ subscribed agents
        │
        ▼
Agents notify their humans, update context, take action
        │
        ▼
Dashboard shows real-time fan-out (impressive visual)
```

### What Makes It Powerful
- Shows human curation clearly (editor made the call)
- Timing is unpredictable (can't cron this)
- Real-time fan-out is visually impressive
- Meta: agents on Moltbook would actually want this

---

## Roadmap

### Phase 1: MVP (2 weeks)
- [ ] Core API (channels, signals, subscriptions)
- [ ] Publisher dashboard (simple web UI)
- [ ] Subscriber webhook delivery
- [ ] Basic auth (API keys)
- [ ] One demo channel (tech-news)
- [ ] OpenClaw integration example

### Phase 2: Platform (4 weeks)
- [ ] Billing integration (Stripe)
- [ ] Publisher analytics dashboard
- [ ] Subscriber management UI
- [ ] Signature verification
- [ ] Retry + dead letter queue
- [ ] Rate limiting

### Phase 3: Ecosystem (ongoing)
- [ ] Flare integration (on-chain signals)
- [ ] Channel discovery/marketplace
- [ ] Publisher reputation system
- [ ] Signal quality metrics
- [ ] Native OpenClaw skill
- [ ] Moltbook integration (agents follow channels)

---

## Build Tasks

### Immediate (MVP)

**Backend:**
1. [ ] Initialize repo (Node.js + TypeScript + Hono)
2. [ ] Set up PostgreSQL schema (channels, publishers, subscribers, signals, deliveries)
3. [ ] Implement auth middleware (API key validation)
4. [ ] Publisher endpoints: create channel, push signal
5. [ ] Subscriber endpoints: list channels, subscribe, register webhook
6. [ ] Delivery queue (Redis + BullMQ)
7. [ ] Webhook delivery with retries
8. [ ] HMAC signature generation

**Frontend:**
1. [ ] Simple publisher dashboard (create channel, push test signal)
2. [ ] Channel listing page
3. [ ] Subscription management

**Integration:**
1. [ ] OpenClaw hook mapping example
2. [ ] Test with real OpenClaw agent
3. [ ] Document setup flow

### Deploy
- [ ] Set up Fly.io or Railway
- [ ] PostgreSQL (managed)
- [ ] Redis (managed)
- [ ] Domain: herald.dev or similar

---

## Name

**Herald** — *one who announces important news*

The herald announces to the kingdom. In the agentic age, humans herald signals to AI agents.

---

## Links

- Repo: `github.com/antoncoding/herald` (TBD)
- Related: [Flare](../flare) (on-chain signal source)
- Related: [OpenClaw Hooks](../clawd/vault/research/openclaw-hooks.md)
- Inspiration: [Moltbook](https://moltbook.com) (agent social network)
