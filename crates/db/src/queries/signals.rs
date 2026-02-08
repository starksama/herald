//! Signal database operations.
//!
//! Signals are the core notification unit in Herald. Publishers create signals
//! on channels, which are then delivered to all channel subscribers.

use crate::models::{Signal, SignalStatus, SignalUrgency};
use sqlx::PgPool;

/// Create a new signal on a channel.
///
/// Returns the created signal with delivery counts initialized to zero.
pub async fn create(
    pool: &PgPool,
    id: &str,
    channel_id: &str,
    title: &str,
    body: &str,
    urgency: SignalUrgency,
    metadata: serde_json::Value,
) -> Result<Signal, sqlx::Error> {
    sqlx::query_as::<_, Signal>(
        r#"
        INSERT INTO signals (id, channel_id, title, body, urgency, metadata)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, channel_id, title, body, urgency, metadata,
                  delivery_count, delivered_count, failed_count, status, created_at
        "#,
    )
    .bind(id)
    .bind(channel_id)
    .bind(title)
    .bind(body)
    .bind(urgency)
    .bind(metadata)
    .fetch_one(pool)
    .await
}

/// Fetch a signal by its unique ID.
pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Signal>, sqlx::Error> {
    sqlx::query_as::<_, Signal>(
        r#"
        SELECT id, channel_id, title, body, urgency, metadata,
               delivery_count, delivered_count, failed_count, status, created_at
        FROM signals
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// List signals for a channel with cursor-based pagination.
///
/// Returns signals ordered by creation date (newest first).
/// Use the last signal's ID as the cursor for the next page.
pub async fn list_by_channel(
    pool: &PgPool,
    channel_id: &str,
    limit: i64,
    cursor: Option<&str>,
) -> Result<Vec<Signal>, sqlx::Error> {
    if let Some(cursor) = cursor {
        sqlx::query_as::<_, Signal>(
            r#"
            SELECT id, channel_id, title, body, urgency, metadata,
                   delivery_count, delivered_count, failed_count, status, created_at
            FROM signals
            WHERE channel_id = $1 AND id < $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(channel_id)
        .bind(cursor)
        .bind(limit)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, Signal>(
            r#"
            SELECT id, channel_id, title, body, urgency, metadata,
                   delivery_count, delivered_count, failed_count, status, created_at
            FROM signals
            WHERE channel_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(channel_id)
        .bind(limit)
        .fetch_all(pool)
        .await
    }
}

/// Update a signal's status (e.g., to mark as deleted).
pub async fn update_status(
    pool: &PgPool,
    id: &str,
    status: SignalStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE signals
        SET status = $1
        WHERE id = $2
        "#,
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Atomically update delivery statistics for a signal.
///
/// Called by the delivery worker after each delivery attempt to track
/// success/failure rates across all subscribers.
pub async fn increment_delivery_counts(
    pool: &PgPool,
    signal_id: &str,
    delivered_delta: i32,
    failed_delta: i32,
    total_delta: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE signals
        SET delivered_count = delivered_count + $1,
            failed_count = failed_count + $2,
            delivery_count = delivery_count + $3
        WHERE id = $4
        "#,
    )
    .bind(delivered_delta)
    .bind(failed_delta)
    .bind(total_delta)
    .bind(signal_id)
    .execute(pool)
    .await?;
    Ok(())
}
