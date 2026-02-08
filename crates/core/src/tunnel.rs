use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::types::SignalUrgency;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Auth { token: String },
    Ack { delivery_id: String },
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    AuthOk {
        connection_id: String,
        subscriber_id: String,
    },
    AuthError {
        message: String,
    },
    Signal {
        delivery_id: String,
        channel_id: String,
        channel_slug: String,
        signal: TunnelSignal,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelSignal {
    pub id: String,
    pub title: String,
    pub body: String,
    pub urgency: SignalUrgency,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct AgentConnection {
    pub connection_id: String,
    pub subscriber_id: String,
    pub sender: mpsc::Sender<ServerMessage>,
    pub connected_at: DateTime<Utc>,
}

#[derive(Default)]
pub struct AgentRegistry {
    agents: RwLock<HashMap<String, Arc<AgentConnection>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, conn: AgentConnection) {
        let subscriber_id = conn.subscriber_id.clone();
        self.agents
            .write()
            .await
            .insert(subscriber_id, Arc::new(conn));
    }

    pub async fn unregister(&self, subscriber_id: &str) {
        self.agents.write().await.remove(subscriber_id);
    }

    pub async fn get(&self, subscriber_id: &str) -> Option<Arc<AgentConnection>> {
        self.agents.read().await.get(subscriber_id).cloned()
    }
}

pub static AGENT_REGISTRY: Lazy<Arc<AgentRegistry>> = Lazy::new(|| Arc::new(AgentRegistry::new()));
