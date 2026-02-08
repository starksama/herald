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
CREATE TYPE delivery_mode AS ENUM ('agent', 'webhook');
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
  delivery_mode delivery_mode NOT NULL DEFAULT 'agent',
  agent_last_connected_at TIMESTAMPTZ,
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
  webhook_id TEXT REFERENCES webhooks(id) ON DELETE RESTRICT,
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
  webhook_id TEXT REFERENCES webhooks(id) ON DELETE CASCADE,
  delivery_mode delivery_mode NOT NULL,
  attempt INTEGER NOT NULL,
  status delivery_status NOT NULL DEFAULT 'pending',
  status_code INTEGER,
  error_message TEXT,
  latency_ms INTEGER,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- agent_connections
CREATE TABLE agent_connections (
  id TEXT PRIMARY KEY,
  subscriber_id TEXT NOT NULL REFERENCES subscribers(id) ON DELETE CASCADE,
  connection_id TEXT NOT NULL,
  server_id TEXT NOT NULL,
  connected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  disconnected_at TIMESTAMPTZ,
  disconnect_reason TEXT,
  signals_delivered INTEGER NOT NULL DEFAULT 0
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
CREATE INDEX idx_agent_connections_subscriber ON agent_connections (subscriber_id, connected_at DESC);
CREATE UNIQUE INDEX idx_api_keys_hash_active ON api_keys (key_hash)
  WHERE status = 'active';
