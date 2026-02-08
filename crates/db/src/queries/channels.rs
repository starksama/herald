use crate::models::{Channel, ChannelStatus, PricingTier};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, QueryBuilder};

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    id: &str,
    publisher_id: &str,
    slug: &str,
    display_name: &str,
    description: Option<&str>,
    category: Option<&str>,
    pricing_tier: PricingTier,
    price_cents: i32,
    is_public: bool,
) -> Result<Channel, sqlx::Error> {
    sqlx::query_as::<_, Channel>(
        r#"
        INSERT INTO channels
            (id, publisher_id, slug, display_name, description, category,
             pricing_tier, price_cents, is_public)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, publisher_id, slug, display_name, description, category,
                  pricing_tier, price_cents, status, is_public,
                  signal_count, subscriber_count, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(publisher_id)
    .bind(slug)
    .bind(display_name)
    .bind(description)
    .bind(category)
    .bind(pricing_tier)
    .bind(price_cents)
    .bind(is_public)
    .fetch_one(pool)
    .await
}

pub async fn get_by_id(pool: &PgPool, id: &str) -> Result<Option<Channel>, sqlx::Error> {
    sqlx::query_as::<_, Channel>(
        r#"
        SELECT id, publisher_id, slug, display_name, description, category,
               pricing_tier, price_cents, status, is_public,
               signal_count, subscriber_count, created_at, updated_at
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_marketplace(pool: &PgPool) -> Result<Vec<Channel>, sqlx::Error> {
    sqlx::query_as::<_, Channel>(
        r#"
        SELECT id, publisher_id, slug, display_name, description, category,
               pricing_tier, price_cents, status, is_public,
               signal_count, subscriber_count, created_at, updated_at
        FROM channels
        WHERE is_public = true AND status = 'active'
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &PgPool,
    id: &str,
    display_name: Option<&str>,
    description: Option<&str>,
    category: Option<&str>,
    pricing_tier: Option<PricingTier>,
    price_cents: Option<i32>,
    is_public: Option<bool>,
    status: Option<ChannelStatus>,
) -> Result<(String, String, DateTime<Utc>), sqlx::Error> {
    let mut qb = QueryBuilder::new("UPDATE channels SET ");
    let mut set = qb.separated(", ");
    let mut updated = false;

    if let Some(value) = display_name {
        set.push("display_name = ").push_bind(value);
        updated = true;
    }
    if let Some(value) = description {
        set.push("description = ").push_bind(value);
        updated = true;
    }
    if let Some(value) = category {
        set.push("category = ").push_bind(value);
        updated = true;
    }
    if let Some(value) = pricing_tier {
        set.push("pricing_tier = ").push_bind(value);
        updated = true;
    }
    if let Some(value) = price_cents {
        set.push("price_cents = ").push_bind(value);
        updated = true;
    }
    if let Some(value) = is_public {
        set.push("is_public = ").push_bind(value);
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
    qb.push(" RETURNING id, display_name, updated_at");

    let record = qb
        .build_query_as::<(String, String, DateTime<Utc>)>()
        .fetch_one(pool)
        .await?;

    Ok(record)
}

pub async fn soft_delete(pool: &PgPool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE channels
        SET status = 'deleted', is_public = false, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn increment_signal_count(
    pool: &PgPool,
    channel_id: &str,
    delta: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE channels
        SET signal_count = signal_count + $1,
            updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(delta)
    .bind(channel_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn increment_subscriber_count(
    pool: &PgPool,
    channel_id: &str,
    delta: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE channels
        SET subscriber_count = subscriber_count + $1,
            updated_at = now()
        WHERE id = $2
        "#,
    )
    .bind(delta)
    .bind(channel_id)
    .execute(pool)
    .await?;
    Ok(())
}
