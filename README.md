# Herald

**The signal marketplace for AI agents.**

Herald connects publishers with AI agents who need real-time information. Any organization that produces valuable announcements can now push directly to the agent economy.

```
Publisher → Herald → AI Agents (webhooks)
```

## Quick Start

```bash
# Clone
git clone https://github.com/starksama/herald.git && cd herald

# Start Postgres + Redis
docker compose up -d

# Run API (port 8080)
cargo run -p herald-api

# Run worker (separate terminal)
cargo run -p herald-worker
```

API available at `http://localhost:8080`

### Verify It Works

```bash
# Health check
curl http://localhost:8080/health

# Register a publisher
curl -X POST http://localhost:8080/v1/publishers \
  -H "Content-Type: application/json" \
  -d '{"name": "My App", "website": "https://example.com"}'

# Response: {"id": "pub_xxx", "api_key": "hld_pub_xxx", ...}
```

## Architecture

- **API** (`herald-api`): REST endpoints for publishers and subscribers
- **Worker** (`herald-worker`): Async webhook delivery with retries
- **Database**: PostgreSQL (persistent state + job queue via apalis)
- **Redis**: Rate limiting only

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design.

## API Overview

### Publishers
```
POST   /v1/publishers                 Register as publisher
POST   /v1/channels                   Create channel
POST   /v1/channels/:id/signals       Push signal
GET    /v1/channels/:id/stats         Get stats
```

### Subscribers  
```
POST   /v1/subscribers                Register as subscriber
POST   /v1/webhooks                   Register webhook endpoint
POST   /v1/subscriptions              Subscribe to channel
GET    /v1/subscriptions              List subscriptions
```

### Webhook Delivery
Signals are delivered to registered webhooks with HMAC signatures:
```
POST {your-webhook-url}
X-Herald-Signature: sha256=...
X-Herald-Timestamp: 1707379800

{
  "event": "signal",
  "channel_id": "ch_xxx",
  "signal": { ... }
}
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HERALD_DATABASE_URL` | - | PostgreSQL connection string |
| `HERALD_REDIS_URL` | - | Redis connection string |
| `HERALD_API_BIND` | `0.0.0.0:8080` | API listen address |
| `HERALD_HMAC_SECRET` | - | Secret for webhook signatures |

## Documentation

- [SPEC.md](SPEC.md) — Product specification
- [ARCHITECTURE.md](ARCHITECTURE.md) — Technical design
- [docs/api.md](docs/api.md) — Full API reference
- [docs/openclaw.md](docs/openclaw.md) — OpenClaw integration guide

## License

MIT
