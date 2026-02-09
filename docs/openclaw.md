# OpenClaw Integration Guide

Use Herald Agent. It is the recommended and simplest way to connect OpenClaw to Herald.

## Quick Start (Recommended)

Herald Agent is a lightweight binary that opens an outbound WebSocket tunnel to Herald. No public endpoint, no firewall changes, no Tailscale.

```bash
# Install herald-agent
curl -fsSL https://herald.dev/install.sh | sh

# Run alongside OpenClaw
herald-agent --token hld_sub_xxx --forward http://localhost:8080/hooks
```

That is it. Done.

## How It Works

1. Agent authenticates with Herald using your subscriber API key.
2. Agent opens a persistent WebSocket connection (outbound only).
3. Herald delivers signals through the tunnel.
4. Agent forwards to your local OpenClaw hook endpoint.

## Subscribe to Channels

```bash
curl -X POST https://api.herald.dev/v1/subscriptions \
  -H "Authorization: Bearer hld_sub_xxx" \
  -H "Content-Type: application/json" \
  -d '{"channelId": "ch_tech_news"}'
```

No webhook registration needed for agent mode.

## Hook Endpoints (OpenClaw)

- `/hooks/wake` injects a message into the main session.
- `/hooks/agent` spawns an isolated session for analysis.

## Security

Herald signs webhook deliveries. This matters only for legacy webhook mode.

```python
import hmac
import hashlib

def verify_herald(payload, signature, timestamp, secret):
    data = f"{timestamp}.{payload}"
    expected = "sha256=" + hmac.new(
        secret.encode(),
        data.encode(),
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(expected, signature)
```

Headers:
- `X-Herald-Signature`
- `X-Herald-Timestamp`
- `X-Herald-Delivery-Id`

## Legacy: Webhook Mode (Not Recommended)

Use only if your environment cannot run Herald Agent and you must expose a public HTTPS endpoint.

### 1. Enable Hooks in OpenClaw

```json
{
  "hooks": {
    "enabled": true,
    "token": "your-shared-secret",
    "path": "/hooks"
  }
}
```

### 2. Register Webhook with Herald

```bash
curl -X POST https://api.herald.dev/v1/webhooks \
  -H "Authorization: Bearer hld_sub_your_key" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "OpenClaw Gateway",
    "url": "https://your-public-endpoint.example.com/hooks/herald",
    "token": "your-shared-secret"
  }'
```

### 3. Subscribe to Channels

```bash
curl -X POST https://api.herald.dev/v1/subscriptions \
  -H "Authorization: Bearer hld_sub_your_key" \
  -H "Content-Type: application/json" \
  -d '{
    "channelId": "ch_tech_news",
    "webhookId": "wh_your_webhook"
  }'
```

Note: If you do not have a public URL, use a network tunnel solution (for example, Tailscale or ngrok) specifically for webhook mode.

## Links

- [OpenClaw Webhook Docs](https://docs.openclaw.ai/automation/webhook)
- [Herald API Reference](./api.md)
