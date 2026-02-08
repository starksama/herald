use anyhow::Context;
use chrono::Utc;
use core::{auth::sign_payload, types::DeliveryJob};
use core::tunnel::{ServerMessage, TunnelSignal};
use core::types::SignalUrgency as CoreSignalUrgency;
use db::models::{DeliveryMode, DeliveryStatus, SignalUrgency};
use serde_json::json;
use std::time::Instant;

use crate::WorkerState;

fn convert_urgency(urgency: &SignalUrgency) -> CoreSignalUrgency {
    match urgency {
        SignalUrgency::Low => CoreSignalUrgency::Low,
        SignalUrgency::Normal => CoreSignalUrgency::Normal,
        SignalUrgency::High => CoreSignalUrgency::High,
        SignalUrgency::Critical => CoreSignalUrgency::Critical,
    }
}

pub fn retry_policy(attempt: u32) -> std::time::Duration {
    match attempt {
        0 => std::time::Duration::from_secs(0),
        1 => std::time::Duration::from_secs(60),
        2 => std::time::Duration::from_secs(300),
        3 => std::time::Duration::from_secs(1800),
        4 => std::time::Duration::from_secs(7200),
        _ => std::time::Duration::from_secs(21600),
    }
}

pub async fn handle_delivery_job(state: &WorkerState, job: DeliveryJob) -> anyhow::Result<()> {
    let signal = db::queries::signals::get_by_id(&state.db, &job.signal_id)
        .await?
        .context("signal not found")?;
    let subscription = db::queries::subscriptions::get_by_id(&state.db, &job.subscription_id)
        .await?
        .context("subscription not found")?;
    let channel = db::queries::channels::get_by_id(&state.db, &signal.channel_id)
        .await?
        .context("channel not found")?;
    let subscriber = db::queries::subscribers::get_by_id(&state.db, &subscription.subscriber_id)
        .await?
        .context("subscriber not found")?;

    if let Some(agent) = state
        .tunnel_registry
        .get(&subscription.subscriber_id)
        .await
    {
        let allow_retry = subscription.webhook_id.is_none();
        if deliver_via_tunnel(
            state,
            &signal,
            &subscription,
            &channel,
            &agent,
            job.attempt,
            allow_retry,
        )
            .await?
        {
            return Ok(());
        }
    }

    if let Some(webhook_id) = subscription.webhook_id.as_deref() {
        let webhook = db::queries::webhooks::get_by_id(&state.db, webhook_id)
            .await?
            .context("webhook not found")?;

        return deliver_via_webhook(
            state,
            &signal,
            &subscription,
            &channel,
            &subscriber,
            &webhook,
            job.attempt,
        )
        .await;
    }

    Err(anyhow::anyhow!("No delivery method available"))
}

#[allow(clippy::too_many_arguments)]
async fn deliver_via_webhook(
    state: &WorkerState,
    signal: &db::models::Signal,
    subscription: &db::models::Subscription,
    channel: &db::models::Channel,
    subscriber: &db::models::Subscriber,
    webhook: &db::models::Webhook,
    attempt: i32,
) -> anyhow::Result<()> {
    let delivery_id = format!("del_{}", nanoid::nanoid!(12));
    let delivery = db::queries::deliveries::create(
        &state.db,
        &delivery_id,
        &signal.id,
        &subscription.id,
        Some(&webhook.id),
        DeliveryMode::Webhook,
        attempt,
    )
    .await?;

    let payload = build_payload(&delivery.id, Some(&webhook.id), channel, signal);

    let body = serde_json::to_string(&payload)?;
    let timestamp = Utc::now().timestamp();
    let signature = sign_payload(&subscriber.webhook_secret, timestamp, &body);

    let mut req = state
        .client
        .post(&webhook.url)
        .header("Content-Type", "application/json")
        .header("X-Herald-Signature", signature)
        .header("X-Herald-Timestamp", timestamp.to_string())
        .header("X-Herald-Delivery-Id", delivery.id.clone());

    if let Some(token) = webhook.token.as_deref() {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let start = Instant::now();
    let result = req.body(body).send().await;
    let latency_ms = start.elapsed().as_millis() as i32;

    match result {
        Ok(resp) => {
            let status_code = resp.status().as_u16() as i32;
            if resp.status().is_success() {
                db::queries::deliveries::update_status(
                    &state.db,
                    &delivery.id,
                    DeliveryStatus::Success,
                    Some(status_code),
                    None,
                    Some(latency_ms),
                )
                .await?;

                db::queries::signals::increment_delivery_counts(&state.db, &signal.id, 1, 0, 1)
                    .await?;

                db::queries::webhooks::update_success(&state.db, &webhook.id, Utc::now()).await?;

                return Ok(());
            }

            let error_message = format!("HTTP {}", status_code);
            handle_webhook_failure(
                state,
                signal,
                subscription,
                webhook,
                &payload,
                delivery.id,
                attempt,
                Some(status_code),
                &error_message,
                latency_ms,
            )
            .await
        }
        Err(err) => {
            handle_webhook_failure(
                state,
                signal,
                subscription,
                webhook,
                &payload,
                delivery.id,
                attempt,
                None,
                &err.to_string(),
                latency_ms,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_webhook_failure(
    state: &WorkerState,
    signal: &db::models::Signal,
    subscription: &db::models::Subscription,
    webhook: &db::models::Webhook,
    payload: &serde_json::Value,
    delivery_id: String,
    attempt: i32,
    status_code: Option<i32>,
    error_message: &str,
    latency_ms: i32,
) -> anyhow::Result<()> {
    db::queries::deliveries::update_status(
        &state.db,
        &delivery_id,
        DeliveryStatus::Failed,
        status_code,
        Some(error_message),
        Some(latency_ms),
    )
    .await?;

    db::queries::signals::increment_delivery_counts(&state.db, &signal.id, 0, 1, 1).await?;
    db::queries::webhooks::update_failure(&state.db, &webhook.id, Utc::now()).await?;

    if attempt >= 5 {
        let error_history = json!([
            {
                "attempt": attempt,
                "error": error_message,
                "statusCode": status_code,
            }
        ]);
        let dlq_id = format!("dlq_{}", nanoid::nanoid!(12));
        db::queries::dead_letter_queue::create(
            &state.db,
            &dlq_id,
            &delivery_id,
            &signal.id,
            &subscription.id,
            payload.clone(),
            error_history,
        )
        .await?;
        return Ok(());
    }

    let queue = match signal.urgency {
        SignalUrgency::High | SignalUrgency::Critical => "delivery-high",
        _ => "delivery-normal",
    };

    let next_job = DeliveryJob {
        signal_id: signal.id.clone(),
        subscription_id: subscription.id.clone(),
        webhook_id: Some(webhook.id.clone()),
        attempt: attempt + 1,
    };

    let delay = retry_policy((attempt + 1) as u32);
    let storage = state.storage.clone();
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        let _ = storage.push(queue, next_job).await;
    });

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn deliver_via_tunnel(
    state: &WorkerState,
    signal: &db::models::Signal,
    subscription: &db::models::Subscription,
    channel: &db::models::Channel,
    agent: &std::sync::Arc<core::tunnel::AgentConnection>,
    attempt: i32,
    allow_retry: bool,
) -> anyhow::Result<bool> {
    let delivery_id = format!("del_{}", nanoid::nanoid!(12));
    let delivery = db::queries::deliveries::create(
        &state.db,
        &delivery_id,
        &signal.id,
        &subscription.id,
        None,
        DeliveryMode::Agent,
        attempt,
    )
    .await?;

    let message = ServerMessage::Signal {
        delivery_id: delivery.id.clone(),
        channel_id: channel.id.clone(),
        channel_slug: channel.slug.clone(),
        signal: TunnelSignal {
            id: signal.id.clone(),
            title: signal.title.clone(),
            body: signal.body.clone(),
            urgency: convert_urgency(&signal.urgency),
            metadata: signal.metadata.clone(),
            created_at: signal.created_at,
        },
    };

    let payload = build_payload(&delivery.id, subscription.webhook_id.as_deref(), channel, signal);

    if let Err(err) = agent.sender.send(message).await {
        handle_tunnel_failure(
            state,
            signal,
            subscription,
            &payload,
            delivery.id,
            attempt,
            &err.to_string(),
            allow_retry,
        )
        .await?;
        return Ok(false);
    }

    db::queries::deliveries::update_status(
        &state.db,
        &delivery.id,
        DeliveryStatus::Success,
        None,
        None,
        None,
    )
    .await?;

    db::queries::signals::increment_delivery_counts(&state.db, &signal.id, 1, 0, 1).await?;

    Ok(true)
}

async fn handle_tunnel_failure(
    state: &WorkerState,
    signal: &db::models::Signal,
    subscription: &db::models::Subscription,
    payload: &serde_json::Value,
    delivery_id: String,
    attempt: i32,
    error_message: &str,
    allow_retry: bool,
) -> anyhow::Result<()> {
    db::queries::deliveries::update_status(
        &state.db,
        &delivery_id,
        DeliveryStatus::Failed,
        None,
        Some(error_message),
        None,
    )
    .await?;

    db::queries::signals::increment_delivery_counts(&state.db, &signal.id, 0, 1, 1).await?;

    if !allow_retry {
        return Ok(());
    }

    if attempt >= 5 {
        let error_history = json!([
            {
                "attempt": attempt,
                "error": error_message,
                "statusCode": null,
            }
        ]);
        let dlq_id = format!("dlq_{}", nanoid::nanoid!(12));
        db::queries::dead_letter_queue::create(
            &state.db,
            &dlq_id,
            &delivery_id,
            &signal.id,
            &subscription.id,
            payload.clone(),
            error_history,
        )
        .await?;
        return Ok(());
    }

    let queue = match signal.urgency {
        SignalUrgency::High | SignalUrgency::Critical => "delivery-high",
        _ => "delivery-normal",
    };

    let next_job = DeliveryJob {
        signal_id: signal.id.clone(),
        subscription_id: subscription.id.clone(),
        webhook_id: subscription.webhook_id.clone(),
        attempt: attempt + 1,
    };

    let delay = retry_policy((attempt + 1) as u32);
    let storage = state.storage.clone();
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        let _ = storage.push(queue, next_job).await;
    });

    Ok(())
}

fn build_payload(
    delivery_id: &str,
    webhook_id: Option<&str>,
    channel: &db::models::Channel,
    signal: &db::models::Signal,
) -> serde_json::Value {
    json!({
        "deliveryId": delivery_id,
        "webhookId": webhook_id,
        "channel": {
            "id": &channel.id,
            "slug": &channel.slug,
            "displayName": &channel.display_name,
        },
        "signal": {
            "id": &signal.id,
            "title": &signal.title,
            "body": &signal.body,
            "urgency": &signal.urgency,
            "metadata": &signal.metadata,
            "createdAt": &signal.created_at,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_retry_policy_immediate_first_attempt() {
        assert_eq!(retry_policy(0), Duration::from_secs(0));
    }

    #[test]
    fn test_retry_policy_one_minute_second_attempt() {
        assert_eq!(retry_policy(1), Duration::from_secs(60));
    }

    #[test]
    fn test_retry_policy_exponential_backoff() {
        assert_eq!(retry_policy(2), Duration::from_secs(300));    // 5 min
        assert_eq!(retry_policy(3), Duration::from_secs(1800));   // 30 min
        assert_eq!(retry_policy(4), Duration::from_secs(7200));   // 2 hours
    }

    #[test]
    fn test_retry_policy_max_backoff() {
        // After attempt 5, should cap at 6 hours
        assert_eq!(retry_policy(5), Duration::from_secs(21600));
        assert_eq!(retry_policy(6), Duration::from_secs(21600));
        assert_eq!(retry_policy(100), Duration::from_secs(21600));
    }

    #[test]
    fn test_convert_urgency_all_levels() {
        assert_eq!(convert_urgency(&SignalUrgency::Low), CoreSignalUrgency::Low);
        assert_eq!(convert_urgency(&SignalUrgency::Normal), CoreSignalUrgency::Normal);
        assert_eq!(convert_urgency(&SignalUrgency::High), CoreSignalUrgency::High);
        assert_eq!(convert_urgency(&SignalUrgency::Critical), CoreSignalUrgency::Critical);
    }

    #[test]
    fn test_queue_selection_for_urgent_signals() {
        // High and Critical should go to delivery-high queue
        assert_eq!(
            match SignalUrgency::High {
                SignalUrgency::High | SignalUrgency::Critical => "delivery-high",
                _ => "delivery-normal",
            },
            "delivery-high"
        );
        assert_eq!(
            match SignalUrgency::Critical {
                SignalUrgency::High | SignalUrgency::Critical => "delivery-high",
                _ => "delivery-normal",
            },
            "delivery-high"
        );
    }

    #[test]
    fn test_queue_selection_for_normal_signals() {
        // Low and Normal should go to delivery-normal queue
        assert_eq!(
            match SignalUrgency::Low {
                SignalUrgency::High | SignalUrgency::Critical => "delivery-high",
                _ => "delivery-normal",
            },
            "delivery-normal"
        );
        assert_eq!(
            match SignalUrgency::Normal {
                SignalUrgency::High | SignalUrgency::Critical => "delivery-high",
                _ => "delivery-normal",
            },
            "delivery-normal"
        );
    }
}
