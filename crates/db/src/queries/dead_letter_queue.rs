use crate::models::DeadLetterEntry;
use sqlx::PgPool;

pub async fn create(
    pool: &PgPool,
    id: &str,
    delivery_id: &str,
    signal_id: &str,
    subscription_id: &str,
    payload: serde_json::Value,
    error_history: serde_json::Value,
) -> Result<DeadLetterEntry, sqlx::Error> {
    sqlx::query_as::<_, DeadLetterEntry>(
        r#"
        INSERT INTO dead_letter_queue
            (id, delivery_id, signal_id, subscription_id, payload, error_history)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, delivery_id, signal_id, subscription_id, payload,
                  error_history, resolved_at, created_at
        "#,
    )
    .bind(id)
    .bind(delivery_id)
    .bind(signal_id)
    .bind(subscription_id)
    .bind(payload)
    .bind(error_history)
    .fetch_one(pool)
    .await
}

pub async fn list_unresolved(pool: &PgPool) -> Result<Vec<DeadLetterEntry>, sqlx::Error> {
    sqlx::query_as::<_, DeadLetterEntry>(
        r#"
        SELECT id, delivery_id, signal_id, subscription_id, payload,
               error_history, resolved_at, created_at
        FROM dead_letter_queue
        WHERE resolved_at IS NULL
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<DeadLetterEntry>, sqlx::Error> {
    sqlx::query_as::<_, DeadLetterEntry>(
        r#"
        SELECT id, delivery_id, signal_id, subscription_id, payload,
               error_history, resolved_at, created_at
        FROM dead_letter_queue
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn resolve(pool: &PgPool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE dead_letter_queue
        SET resolved_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
