//! Delivery database operations.
//!
//! Deliveries track individual attempts to send a signal to a subscriber,
//! either via webhook or agent tunnel.

use crate::models::{Delivery, DeliveryMode, DeliveryStatus};
use sqlx::PgPool;

/// Create a new delivery record for a signal-subscription pair.
///
/// Returns the delivery with status initialized to 'pending'.
pub async fn create(
    pool: &PgPool,
    id: &str,
    signal_id: &str,
    subscription_id: &str,
    webhook_id: Option<&str>,
    delivery_mode: DeliveryMode,
    attempt: i32,
) -> Result<Delivery, sqlx::Error> {
    sqlx::query_as::<_, Delivery>(
        r#"
        INSERT INTO deliveries (id, signal_id, subscription_id, webhook_id, delivery_mode, attempt)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, signal_id, subscription_id, webhook_id, delivery_mode, attempt,
                  status, status_code, error_message, latency_ms,
                  created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(signal_id)
    .bind(subscription_id)
    .bind(webhook_id)
    .bind(delivery_mode)
    .bind(attempt)
    .fetch_one(pool)
    .await
}

/// Update a delivery's status after an attempt completes.
///
/// Records the HTTP status code (for webhooks), any error message,
/// and the round-trip latency for monitoring.
pub async fn update_status(
    pool: &PgPool,
    id: &str,
    status: DeliveryStatus,
    status_code: Option<i32>,
    error_message: Option<&str>,
    latency_ms: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE deliveries
        SET status = $1,
            status_code = $2,
            error_message = $3,
            latency_ms = $4,
            updated_at = now()
        WHERE id = $5
        "#,
    )
    .bind(status)
    .bind(status_code)
    .bind(error_message)
    .bind(latency_ms)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// List deliveries for a specific webhook with cursor-based pagination.
///
/// Returns deliveries ordered by creation date (newest first).
pub async fn list_by_webhook(
    pool: &PgPool,
    webhook_id: &str,
    limit: i64,
    cursor: Option<&str>,
) -> Result<Vec<Delivery>, sqlx::Error> {
    if let Some(cursor) = cursor {
        sqlx::query_as::<_, Delivery>(
            r#"
            SELECT id, signal_id, subscription_id, webhook_id, delivery_mode, attempt,
                   status, status_code, error_message, latency_ms,
                   created_at, updated_at
            FROM deliveries
            WHERE webhook_id = $1 AND id < $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(webhook_id)
        .bind(cursor)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, Delivery>(
            r#"
            SELECT id, signal_id, subscription_id, webhook_id, delivery_mode, attempt,
                   status, status_code, error_message, latency_ms,
                   created_at, updated_at
            FROM deliveries
            WHERE webhook_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(webhook_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

/// List all deliveries for a specific signal (across all subscribers).
pub async fn list_by_signal(pool: &PgPool, signal_id: &str) -> Result<Vec<Delivery>, sqlx::Error> {
    sqlx::query_as::<_, Delivery>(
        r#"
        SELECT id, signal_id, subscription_id, webhook_id, delivery_mode, attempt,
               status, status_code, error_message, latency_ms,
               created_at, updated_at
        FROM deliveries
        WHERE signal_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(signal_id)
    .fetch_all(pool)
    .await
}

/// Fetch a delivery by its unique ID.
pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Delivery>, sqlx::Error> {
    sqlx::query_as::<_, Delivery>(
        r#"
        SELECT id, signal_id, subscription_id, webhook_id, delivery_mode, attempt,
               status, status_code, error_message, latency_ms,
               created_at, updated_at
        FROM deliveries
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}
