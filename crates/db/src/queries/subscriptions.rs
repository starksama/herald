use crate::models::{Subscription, SubscriptionStatus};
use sqlx::PgPool;

pub async fn create(
    pool: &PgPool,
    id: &str,
    subscriber_id: &str,
    channel_id: &str,
    webhook_id: Option<&str>,
) -> Result<Subscription, sqlx::Error> {
    sqlx::query_as::<_, Subscription>(
        r#"
        INSERT INTO subscriptions (id, subscriber_id, channel_id, webhook_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, subscriber_id, channel_id, webhook_id, status,
                  stripe_subscription_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(subscriber_id)
    .bind(channel_id)
    .bind(webhook_id)
    .fetch_one(pool)
    .await
}

pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Subscription>, sqlx::Error> {
    sqlx::query_as::<_, Subscription>(
        r#"
        SELECT id, subscriber_id, channel_id, webhook_id, status,
               stripe_subscription_id, created_at, updated_at
        FROM subscriptions
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_by_subscriber(
    pool: &PgPool,
    subscriber_id: &str,
) -> Result<Vec<Subscription>, sqlx::Error> {
    sqlx::query_as::<_, Subscription>(
        r#"
        SELECT id, subscriber_id, channel_id, webhook_id, status,
               stripe_subscription_id, created_at, updated_at
        FROM subscriptions
        WHERE subscriber_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(subscriber_id)
    .fetch_all(pool)
    .await
}

pub async fn list_active_by_channel(
    pool: &PgPool,
    channel_id: &str,
) -> Result<Vec<Subscription>, sqlx::Error> {
    sqlx::query_as::<_, Subscription>(
        r#"
        SELECT id, subscriber_id, channel_id, webhook_id, status,
               stripe_subscription_id, created_at, updated_at
        FROM subscriptions
        WHERE channel_id = $1 AND status = 'active'
        "#,
    )
    .bind(channel_id)
    .fetch_all(pool)
    .await
}

pub async fn update_status(
    pool: &PgPool,
    id: &str,
    status: SubscriptionStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE subscriptions
        SET status = $1, updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
