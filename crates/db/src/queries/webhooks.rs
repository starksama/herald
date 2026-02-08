use crate::models::{Webhook, WebhookStatus};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub async fn create(
    pool: &PgPool,
    id: &str,
    subscriber_id: &str,
    url: &str,
    name: &str,
    token: Option<&str>,
) -> Result<Webhook, sqlx::Error> {
    sqlx::query_as::<_, Webhook>(
        r#"
        INSERT INTO webhooks (id, subscriber_id, url, name, token)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, subscriber_id, url, name, token, status,
                  failure_count, last_success_at, last_failure_at,
                  created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(subscriber_id)
    .bind(url)
    .bind(name)
    .bind(token)
    .fetch_one(pool)
    .await
}

pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Webhook>, sqlx::Error> {
    sqlx::query_as::<_, Webhook>(
        r#"
        SELECT id, subscriber_id, url, name, token, status,
               failure_count, last_success_at, last_failure_at,
               created_at, updated_at
        FROM webhooks
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
) -> Result<Vec<Webhook>, sqlx::Error> {
    sqlx::query_as::<_, Webhook>(
        r#"
        SELECT id, subscriber_id, url, name, token, status,
               failure_count, last_success_at, last_failure_at,
               created_at, updated_at
        FROM webhooks
        WHERE subscriber_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(subscriber_id)
    .fetch_all(pool)
    .await
}

pub async fn update(
    pool: &PgPool,
    id: &str,
    name: Option<&str>,
    url: Option<&str>,
    status: Option<WebhookStatus>,
) -> Result<(String, WebhookStatus, DateTime<Utc>), sqlx::Error> {
    let mut qb = sqlx::QueryBuilder::new("UPDATE webhooks SET ");
    let mut set = qb.separated(", ");
    let mut updated = false;

    if let Some(value) = name {
        set.push("name = ").push_bind(value);
        updated = true;
    }
    if let Some(value) = url {
        set.push("url = ").push_bind(value);
        updated = true;
    }
    if let Some(value) = status {
        set.push("status = ").push_bind(value);
        updated = true;
    }

    if !updated {
        return Err(sqlx::Error::Protocol("no fields to update".into()));
    }

    set.push("updated_at = now()");
    qb.push(" WHERE id = ").push_bind(id);
    qb.push(" RETURNING id, status, updated_at");

    let record = qb
        .build_query_as::<(String, WebhookStatus, DateTime<Utc>)>()
        .fetch_one(pool)
        .await?;

    Ok(record)
}

pub async fn update_failure(
    pool: &PgPool,
    id: &str,
    last_failure_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE webhooks
        SET failure_count = failure_count + 1,
            last_failure_at = $1,
            updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(last_failure_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_success(
    pool: &PgPool,
    id: &str,
    last_success_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE webhooks
        SET failure_count = 0,
            last_success_at = $1,
            updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(last_success_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
