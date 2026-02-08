use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    Extension,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::{
    state::{AppState, RequestId},
    tunnel::protocol::{ClientMessage, ServerMessage, TunnelSignal},
    tunnel::registry::AgentConnection,
};
use core::auth::hash_api_key;
use core::types::SignalUrgency as CoreSignalUrgency;
use db::models::{ApiKeyOwner, SignalUrgency};

pub async fn tunnel_ws(
    State(state): State<AppState>,
    Extension(request_id): Extension<RequestId>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(state, socket, request_id))
}

async fn handle_socket(state: AppState, socket: WebSocket, request_id: RequestId) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<ServerMessage>(64);

    let send_task = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            let text = match serde_json::to_string(&msg) {
                Ok(text) => text,
                Err(err) => {
                    warn!(error = %err, "tunnel: failed to serialize message");
                    continue;
                }
            };

            if ws_sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    let auth_msg = match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => serde_json::from_str::<ClientMessage>(&text).ok(),
        Some(Ok(Message::Binary(bytes))) => {
            serde_json::from_slice::<ClientMessage>(&bytes).ok()
        }
        _ => None,
    };

    let (subscriber_id, connection_id) = match auth_msg {
        Some(ClientMessage::Auth { token }) => {
            match authenticate(&state, &token, &request_id).await {
                Ok(subscriber_id) => {
                    let connection_id = format!("conn_{}", nanoid::nanoid!(12));
                    (subscriber_id, connection_id)
                }
                Err(message) => {
                    let _ = outbound_tx
                        .send(ServerMessage::AuthError { message })
                        .await;
                    drop(outbound_tx);
                    let _ = send_task.await;
                    return;
                }
            }
        }
        _ => {
            let _ = outbound_tx
                .send(ServerMessage::AuthError {
                    message: "invalid auth payload".to_string(),
                })
                .await;
            drop(outbound_tx);
            let _ = send_task.await;
            return;
        }
    };

    let conn = AgentConnection {
        connection_id: connection_id.clone(),
        subscriber_id: subscriber_id.clone(),
        sender: outbound_tx.clone(),
        connected_at: Utc::now(),
    };
    state.tunnel_registry.register(conn).await;

    let _ = db::queries::subscribers::update_agent_last_connected_at(
        &state.db,
        &subscriber_id,
        Utc::now(),
    )
    .await;

    let _ = outbound_tx
        .send(ServerMessage::AuthOk {
            connection_id: connection_id.clone(),
            subscriber_id: subscriber_id.clone(),
        })
        .await;

    let ping_tx = outbound_tx.clone();
    let ping_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if ping_tx.send(ServerMessage::Ping).await.is_err() {
                break;
            }
        }
    });

    info!(
        subscriber_id = %subscriber_id,
        connection_id = %connection_id,
        "tunnel connected"
    );

    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(Message::Text(text)) => handle_client_message(&subscriber_id, &text).await,
            Ok(Message::Binary(bytes)) => {
                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                    handle_client_message(&subscriber_id, &text).await;
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
            Err(err) => {
                warn!(error = %err, "tunnel receive error");
                break;
            }
        }
    }

    state.tunnel_registry.unregister(&subscriber_id).await;
    ping_task.abort();
    drop(outbound_tx);
    let _ = send_task.await;

    info!(
        subscriber_id = %subscriber_id,
        connection_id = %connection_id,
        "tunnel disconnected"
    );
}

async fn authenticate(
    state: &AppState,
    token: &str,
    request_id: &RequestId,
) -> Result<String, String> {
    if token.is_empty() {
        return Err("missing token".to_string());
    }

    let hash = hash_api_key(token);
    let api_key = db::queries::api_keys::get_by_hash(&state.db, &hash)
        .await
        .map_err(|err| {
            error!(error = %err, request_id = %request_id.0, "tunnel auth lookup failed");
            "internal auth error".to_string()
        })?
        .ok_or_else(|| "invalid token".to_string())?;

    if api_key.owner_type != ApiKeyOwner::Subscriber {
        return Err("subscriber token required".to_string());
    }

    Ok(api_key.owner_id)
}

async fn handle_client_message(subscriber_id: &str, text: &str) {
    let Ok(message) = serde_json::from_str::<ClientMessage>(text) else {
        warn!(subscriber_id = %subscriber_id, "tunnel: invalid client message");
        return;
    };

    match message {
        ClientMessage::Ack { delivery_id } => {
            info!(
                subscriber_id = %subscriber_id,
                delivery_id = %delivery_id,
                "tunnel delivery acknowledged"
            );
        }
        ClientMessage::Pong => {}
        ClientMessage::Auth { .. } => {
            warn!(subscriber_id = %subscriber_id, "tunnel: unexpected auth message");
        }
    }
}

fn convert_urgency(urgency: &SignalUrgency) -> CoreSignalUrgency {
    match urgency {
        SignalUrgency::Low => CoreSignalUrgency::Low,
        SignalUrgency::Normal => CoreSignalUrgency::Normal,
        SignalUrgency::High => CoreSignalUrgency::High,
        SignalUrgency::Critical => CoreSignalUrgency::Critical,
    }
}

pub fn to_tunnel_signal(signal: &db::models::Signal) -> TunnelSignal {
    TunnelSignal {
        id: signal.id.clone(),
        title: signal.title.clone(),
        body: signal.body.clone(),
        urgency: convert_urgency(&signal.urgency),
        metadata: signal.metadata.clone(),
        created_at: signal.created_at,
    }
}
