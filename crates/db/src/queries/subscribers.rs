use crate::models::Subscriber;
use sqlx::PgPool;

pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Subscriber>, sqlx::Error> {
    sqlx::query_as::<_, Subscriber>(
        r#"
        SELECT id, name, email, webhook_secret, stripe_customer_id,
               tier, status, delivery_mode, agent_last_connected_at,
               created_at, updated_at
        FROM subscribers
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_by_email(pool: &PgPool, email: &str) -> Result<Option<Subscriber>, sqlx::Error> {
    sqlx::query_as::<_, Subscriber>(
        r#"
        SELECT id, name, email, webhook_secret, stripe_customer_id,
               tier, status, delivery_mode, agent_last_connected_at,
               created_at, updated_at
        FROM subscribers
        WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn update_agent_last_connected_at(
    pool: &PgPool,
    id: &str,
    connected_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE subscribers
        SET agent_last_connected_at = $1, updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(connected_at)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
