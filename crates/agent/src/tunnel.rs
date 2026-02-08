use backoff::{backoff::Backoff, ExponentialBackoff};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use core::tunnel::{ClientMessage, ServerMessage};

use crate::config::AgentConfig;
use crate::forward::Forwarder;

pub async fn run_tunnel(config: AgentConfig) -> anyhow::Result<()> {
    let mut backoff = ExponentialBackoff {
        max_elapsed_time: None,
        ..Default::default()
    };

    loop {
        match connect_and_run(&config).await {
            Ok(()) => {
                info!("tunnel disconnected cleanly");
                backoff.reset();
            }
            Err(err) => {
                error!(error = %err, "tunnel error");
            }
        }

        let delay = backoff
            .next_backoff()
            .unwrap_or_else(|| std::time::Duration::from_secs(60));
        info!(?delay, "reconnecting");
        tokio::time::sleep(delay).await;
    }
}

async fn connect_and_run(config: &AgentConfig) -> anyhow::Result<()> {
    let (ws_stream, _) = tokio_tungstenite::connect_async(&config.herald_url).await?;
    let (mut write, mut read) = ws_stream.split();

    let auth = ClientMessage::Auth {
        token: config.token.clone(),
    };
    write
        .send(Message::Text(serde_json::to_string(&auth)?))
        .await?;

    let forwarder = Forwarder::new(config.forward_url.clone())?;

    while let Some(message) = read.next().await {
        let message = message?;
        match message {
            Message::Text(text) => {
                handle_server_message(&forwarder, &mut write, &text).await?;
            }
            Message::Binary(bytes) => {
                if let Ok(text) = String::from_utf8(bytes) {
                    handle_server_message(&forwarder, &mut write, &text).await?;
                }
            }
            Message::Close(_) => break,
            Message::Ping(payload) => {
                let _ = write.send(Message::Pong(payload)).await;
            }
            Message::Pong(_) => {}
            _ => {}
        }
    }

    Ok(())
}

async fn handle_server_message(
    forwarder: &Forwarder,
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >,
    text: &str,
) -> anyhow::Result<()> {
    let message: ServerMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(err) => {
            warn!(error = %err, "invalid server message");
            return Ok(());
        }
    };

    match message {
        ServerMessage::AuthOk {
            connection_id,
            subscriber_id,
        } => {
            info!(%connection_id, %subscriber_id, "tunnel authenticated");
        }
        ServerMessage::AuthError { message } => {
            return Err(anyhow::anyhow!(message));
        }
        ServerMessage::Ping => {
            let pong = ClientMessage::Pong;
            write
                .send(Message::Text(serde_json::to_string(&pong)?))
                .await?;
        }
        ServerMessage::Signal {
            delivery_id,
            channel_id,
            channel_slug,
            signal,
        } => {
            match forwarder
                .deliver_signal(&delivery_id, &channel_id, &channel_slug, &signal)
                .await
            {
                Ok(()) => {
                    let ack = ClientMessage::Ack { delivery_id };
                    write
                        .send(Message::Text(serde_json::to_string(&ack)?))
                        .await?;
                }
                Err(err) => {
                    warn!(error = %err, "local forward failed");
                }
            }
        }
    }

    Ok(())
}
