use crate::models::Subscriber;
use sqlx::PgPool;

pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Subscriber>, sqlx::Error> {
    sqlx::query_as::<_, Subscriber>(
        r#"
        SELECT id, name, email, webhook_secret, stripe_customer_id,
               tier, status, created_at, updated_at
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
               tier, status, created_at, updated_at
        FROM subscribers
        WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}
