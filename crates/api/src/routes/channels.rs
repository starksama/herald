use axum::{
    extract::{Path, State},
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;

use crate::{
    error::{ApiError, ApiResult},
    middleware::auth::{AuthContext, OwnerType},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/channels", post(create_channel).get(list_channels))
        .route(
            "/v1/channels/:id",
            get(get_channel).patch(update_channel).delete(delete_channel),
        )
        .route("/v1/channels/:id/stats", get(channel_stats))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateChannelRequest {
    slug: String,
    display_name: String,
    description: Option<String>,
    category: Option<String>,
    pricing_tier: Option<String>,
    price_cents: Option<i32>,
    is_public: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelSummaryResponse {
    id: String,
    slug: String,
    display_name: String,
    pricing_tier: String,
    price_cents: i32,
    subscriber_count: i32,
    signal_count: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelDetailResponse {
    id: String,
    slug: String,
    display_name: String,
    description: Option<String>,
    category: Option<String>,
    pricing_tier: String,
    price_cents: i32,
    status: String,
    is_public: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateChannelResponse {
    id: String,
    display_name: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteChannelResponse {
    id: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelListResponse {
    items: Vec<ChannelListItem>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct ChannelListItem {
    id: String,
    slug: String,
    display_name: String,
    pricing_tier: String,
    price_cents: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelStatsResponse {
    signal_count: i32,
    subscriber_count: i32,
    delivery_success_rate: f64,
}

#[derive(Debug, sqlx::FromRow)]
struct ChannelRow {
    id: String,
    publisher_id: String,
    slug: String,
    display_name: String,
    description: Option<String>,
    category: Option<String>,
    pricing_tier: String,
    price_cents: i32,
    status: String,
    is_public: bool,
    signal_count: i32,
    subscriber_count: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateChannelRequest {
    display_name: Option<String>,
    description: Option<String>,
    category: Option<String>,
    pricing_tier: Option<String>,
    price_cents: Option<i32>,
    is_public: Option<bool>,
    status: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChannelStatsRow {
    signal_count: i32,
    subscriber_count: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct DeliveryTotalsRow {
    delivered: i64,
    total: i64,
}

pub async fn create_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<CreateChannelRequest>,
) -> ApiResult<Json<ChannelSummaryResponse>> {
    let publisher_id = require_publisher(&auth)?;

    if payload.slug.trim().is_empty() || payload.display_name.trim().is_empty() {
        return Err(ApiError::BadRequest("slug and displayName required".to_string()));
    }

    let pricing_tier = payload.pricing_tier.unwrap_or_else(|| "free".to_string());
    let price_cents = payload.price_cents.unwrap_or(0);
    let is_public = payload.is_public.unwrap_or(true);
    let id = format!("ch_{}", nanoid::nanoid!(12));

    let record = sqlx::query_as::<_, ChannelRow>(
        r#"
        INSERT INTO channels (id, publisher_id, slug, display_name, description, category, pricing_tier, price_cents, is_public)
        VALUES ($1, $2, $3, $4, $5, $6, $7::pricing_tier, $8, $9)
        RETURNING id, publisher_id, slug, display_name, description, category,
                  pricing_tier::text as pricing_tier, price_cents, status::text as status,
                  is_public, signal_count, subscriber_count
        "#,
    )
    .bind(&id)
    .bind(publisher_id)
    .bind(&payload.slug)
    .bind(&payload.display_name)
    .bind(payload.description)
    .bind(payload.category)
    .bind(&pricing_tier)
    .bind(price_cents)
    .bind(is_public)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ChannelSummaryResponse {
        id: record.id,
        slug: record.slug,
        display_name: record.display_name,
        pricing_tier: record.pricing_tier,
        price_cents: record.price_cents,
        subscriber_count: record.subscriber_count,
        signal_count: record.signal_count,
    }))
}

pub async fn list_channels(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<ChannelListResponse>> {
    match auth.owner_type {
        OwnerType::Subscriber => {}
        OwnerType::Publisher => {
            return Err(ApiError::Forbidden(
                "publishers cannot list marketplace channels".to_string(),
            ));
        }
    }

    let channels = sqlx::query_as::<_, ChannelListItem>(
        r#"
        SELECT id, slug, display_name, pricing_tier::text as pricing_tier, price_cents
        FROM channels
        WHERE is_public = true AND status = 'active'
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(ChannelListResponse { items: channels }))
}

pub async fn get_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> ApiResult<Json<ChannelDetailResponse>> {
    let channel = sqlx::query_as::<_, ChannelRow>(
        r#"
        SELECT id, publisher_id, slug, display_name, description, category,
               pricing_tier::text as pricing_tier, price_cents,
               status::text as status, is_public, signal_count, subscriber_count
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let channel = match channel {
        Some(channel) => channel,
        None => return Err(ApiError::NotFound("channel not found".to_string())),
    };

    if channel.status == "deleted" {
        return Err(ApiError::NotFound("channel not found".to_string()));
    }

    if !channel.is_public {
        if auth.owner_type != OwnerType::Publisher || channel.publisher_id != auth.owner_id {
            return Err(ApiError::NotFound("channel not found".to_string()));
        }
    }

    Ok(Json(ChannelDetailResponse {
        id: channel.id,
        slug: channel.slug,
        display_name: channel.display_name,
        description: channel.description,
        category: channel.category,
        pricing_tier: channel.pricing_tier,
        price_cents: channel.price_cents,
        status: channel.status,
        is_public: channel.is_public,
    }))
}

pub async fn update_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateChannelRequest>,
) -> ApiResult<Json<UpdateChannelResponse>> {
    let publisher_id = require_publisher(&auth)?;

    let channel = sqlx::query_as::<_, ChannelRow>(
        r#"
        SELECT id, publisher_id, slug, display_name, description, category,
               pricing_tier::text as pricing_tier, price_cents,
               status::text as status, is_public, signal_count, subscriber_count
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let channel = match channel {
        Some(channel) => channel,
        None => return Err(ApiError::NotFound("channel not found".to_string())),
    };

    if channel.publisher_id != publisher_id {
        return Err(ApiError::Forbidden("not channel owner".to_string()));
    }

    let mut qb = QueryBuilder::new("UPDATE channels SET ");
    let mut set = qb.separated(", ");
    let mut updated = false;

    if let Some(display_name) = payload.display_name {
        set.push("display_name = ").push_bind(display_name);
        updated = true;
    }
    if let Some(description) = payload.description {
        set.push("description = ").push_bind(description);
        updated = true;
    }
    if let Some(category) = payload.category {
        set.push("category = ").push_bind(category);
        updated = true;
    }
    if let Some(pricing_tier) = payload.pricing_tier {
        set.push("pricing_tier = ")
            .push_bind(pricing_tier)
            .push("::pricing_tier");
        updated = true;
    }
    if let Some(price_cents) = payload.price_cents {
        set.push("price_cents = ").push_bind(price_cents);
        updated = true;
    }
    if let Some(is_public) = payload.is_public {
        set.push("is_public = ").push_bind(is_public);
        updated = true;
    }
    if let Some(status) = payload.status {
        set.push("status = ")
            .push_bind(status)
            .push("::channel_status");
        updated = true;
    }

    if !updated {
        return Err(ApiError::BadRequest("no fields to update".to_string()));
    }

    set.push("updated_at = now()");
    qb.push(" WHERE id = ").push_bind(&id);
    qb.push(" RETURNING id, display_name, updated_at");

    let record = qb
        .build_query_as::<UpdateChannelRow>()
        .fetch_one(&state.db)
        .await?;

    Ok(Json(UpdateChannelResponse {
        id: record.id,
        display_name: record.display_name,
        updated_at: record.updated_at,
    }))
}

pub async fn delete_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> ApiResult<Json<DeleteChannelResponse>> {
    let publisher_id = require_publisher(&auth)?;

    let record = sqlx::query_as::<_, ChannelOwnerRow>(
        r#"
        SELECT id, publisher_id
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let record = match record {
        Some(record) => record,
        None => return Err(ApiError::NotFound("channel not found".to_string())),
    };

    if record.publisher_id != publisher_id {
        return Err(ApiError::Forbidden("not channel owner".to_string()));
    }

    sqlx::query(
        r#"
        UPDATE channels
        SET status = 'deleted', is_public = false, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .execute(&state.db)
    .await?;

    Ok(Json(DeleteChannelResponse {
        id,
        status: "deleted".to_string(),
    }))
}

pub async fn channel_stats(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> ApiResult<Json<ChannelStatsResponse>> {
    let channel = sqlx::query_as::<_, ChannelRow>(
        r#"
        SELECT id, publisher_id, slug, display_name, description, category,
               pricing_tier::text as pricing_tier, price_cents,
               status::text as status, is_public, signal_count, subscriber_count
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let channel = match channel {
        Some(channel) => channel,
        None => return Err(ApiError::NotFound("channel not found".to_string())),
    };

    if auth.owner_type != OwnerType::Publisher || channel.publisher_id != auth.owner_id {
        return Err(ApiError::Forbidden("not channel owner".to_string()));
    }

    let stats = sqlx::query_as::<_, ChannelStatsRow>(
        r#"
        SELECT signal_count, subscriber_count
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await?;

    let totals = sqlx::query_as::<_, DeliveryTotalsRow>(
        r#"
        SELECT COALESCE(SUM(delivered_count), 0) as delivered,
               COALESCE(SUM(delivery_count), 0) as total
        FROM signals
        WHERE channel_id = $1
        "#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await?;

    let delivery_success_rate = if totals.total > 0 {
        totals.delivered as f64 / totals.total as f64
    } else {
        0.0
    };

    Ok(Json(ChannelStatsResponse {
        signal_count: stats.signal_count,
        subscriber_count: stats.subscriber_count,
        delivery_success_rate,
    }))
}

fn require_publisher(auth: &AuthContext) -> ApiResult<&str> {
    match auth.owner_type {
        OwnerType::Publisher => Ok(auth.owner_id.as_str()),
        OwnerType::Subscriber => Err(ApiError::Forbidden("publisher access required".to_string())),
    }
}

#[derive(Debug, sqlx::FromRow)]
struct UpdateChannelRow {
    id: String,
    display_name: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChannelOwnerRow {
    id: String,
    publisher_id: String,
}
