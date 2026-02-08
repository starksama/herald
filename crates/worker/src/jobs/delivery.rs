use anyhow::Context;
use chrono::Utc;
use core::{auth::sign_payload, types::DeliveryJob};
use db::models::{DeliveryStatus, SignalUrgency};
use serde_json::json;
use std::time::Instant;

use crate::WorkerState;

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
    let webhook = db::queries::webhooks::get_by_id(&state.db, &job.webhook_id)
        .await?
        .context("webhook not found")?;
    let channel = db::queries::channels::get_by_id(&state.db, &signal.channel_id)
        .await?
        .context("channel not found")?;
    let subscriber = db::queries::subscribers::get_by_id(&state.db, &subscription.subscriber_id)
        .await?
        .context("subscriber not found")?;

    let delivery_id = format!("del_{}", nanoid::nanoid!(12));
    let delivery = db::queries::deliveries::create(
        &state.db,
        &delivery_id,
        &signal.id,
        &subscription.id,
        &webhook.id,
        job.attempt,
    )
    .await?;

    let payload = json!({
        "deliveryId": delivery.id,
        "webhookId": webhook.id,
        "channel": {
            "id": channel.id,
            "slug": channel.slug,
            "displayName": channel.display_name,
        },
        "signal": {
            "id": signal.id,
            "title": signal.title,
            "body": signal.body,
            "urgency": signal.urgency,
            "metadata": signal.metadata,
            "createdAt": signal.created_at,
        }
    });

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
            handle_failure(
                state,
                &signal,
                &subscription,
                &webhook,
                &payload,
                delivery.id,
                job.attempt,
                Some(status_code),
                &error_message,
                latency_ms,
            )
            .await
        }
        Err(err) => {
            handle_failure(
                state,
                &signal,
                &subscription,
                &webhook,
                &payload,
                delivery.id,
                job.attempt,
                None,
                &err.to_string(),
                latency_ms,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_failure(
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
        webhook_id: webhook.id.clone(),
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
