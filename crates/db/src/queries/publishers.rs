use crate::models::Publisher;
use sqlx::PgPool;

pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Publisher>, sqlx::Error> {
    sqlx::query_as::<_, Publisher>(
        r#"
        SELECT id, name, email, stripe_customer_id, stripe_connect_id,
               tier, status, created_at, updated_at
        FROM publishers
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_by_email(pool: &PgPool, email: &str) -> Result<Option<Publisher>, sqlx::Error> {
    sqlx::query_as::<_, Publisher>(
        r#"
        SELECT id, name, email, stripe_customer_id, stripe_connect_id,
               tier, status, created_at, updated_at
        FROM publishers
        WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}
