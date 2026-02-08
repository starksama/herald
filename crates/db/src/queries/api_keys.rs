use crate::models::{ApiKey, ApiKeyOwner, ApiKeyStatus};
use sqlx::PgPool;

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    id: &str,
    key_hash: &str,
    key_prefix: &str,
    owner_type: ApiKeyOwner,
    owner_id: &str,
    name: Option<&str>,
    scopes: &[String],
) -> Result<ApiKey, sqlx::Error> {
    sqlx::query_as::<_, ApiKey>(
        r#"
        INSERT INTO api_keys
            (id, key_hash, key_prefix, owner_type, owner_id, name, scopes)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, key_hash, key_prefix, owner_type, owner_id, name,
                  scopes, last_used_at, expires_at, status, created_at
        "#,
    )
    .bind(id)
    .bind(key_hash)
    .bind(key_prefix)
    .bind(owner_type)
    .bind(owner_id)
    .bind(name)
    .bind(scopes)
    .fetch_one(pool)
    .await
}

pub async fn get_by_hash(pool: &PgPool, key_hash: &str) -> Result<Option<ApiKey>, sqlx::Error> {
    sqlx::query_as::<_, ApiKey>(
        r#"
        SELECT id, key_hash, key_prefix, owner_type, owner_id, name,
               scopes, last_used_at, expires_at, status, created_at
        FROM api_keys
        WHERE key_hash = $1 AND status = 'active'
        "#,
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
}

pub async fn list_by_owner(
    pool: &PgPool,
    owner_type: ApiKeyOwner,
    owner_id: &str,
) -> Result<Vec<ApiKey>, sqlx::Error> {
    sqlx::query_as::<_, ApiKey>(
        r#"
        SELECT id, key_hash, key_prefix, owner_type, owner_id, name,
               scopes, last_used_at, expires_at, status, created_at
        FROM api_keys
        WHERE owner_type = $1 AND owner_id = $2
        ORDER BY created_at DESC
        "#,
    )
    .bind(owner_type)
    .bind(owner_id)
    .fetch_all(pool)
    .await
}

pub async fn revoke(pool: &PgPool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE api_keys
        SET status = 'revoked'
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn touch_last_used(pool: &PgPool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE api_keys
        SET last_used_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_status(
    pool: &PgPool,
    id: &str,
    status: ApiKeyStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE api_keys
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
