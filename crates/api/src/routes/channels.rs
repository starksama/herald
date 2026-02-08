use axum::{
    extract::{Path, State},
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, ApiResult, AppError},
    middleware::auth::AuthContext,
    state::{AppState, RequestId},
};
use db::models::{ChannelStatus, PricingTier};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/channels", post(create_channel).get(list_channels))
        .route(
            "/v1/channels/:id",
            get(get_channel)
                .patch(update_channel)
                .delete(delete_channel),
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
    pricing_tier: Option<PricingTier>,
    price_cents: Option<i32>,
    is_public: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateChannelRequest {
    display_name: Option<String>,
    description: Option<String>,
    category: Option<String>,
    pricing_tier: Option<PricingTier>,
    price_cents: Option<i32>,
    is_public: Option<bool>,
    status: Option<ChannelStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelSummaryResponse {
    id: String,
    slug: String,
    display_name: String,
    pricing_tier: PricingTier,
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
    pricing_tier: PricingTier,
    price_cents: i32,
    status: ChannelStatus,
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
    status: ChannelStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelListResponse {
    items: Vec<ChannelListItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelListItem {
    id: String,
    slug: String,
    display_name: String,
    pricing_tier: PricingTier,
    price_cents: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelStatsResponse {
    signal_count: i32,
    subscriber_count: i32,
    delivery_success_rate: f64,
}

async fn create_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Json(payload): Json<CreateChannelRequest>,
) -> ApiResult<Json<ChannelSummaryResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    if payload.slug.trim().is_empty() || payload.display_name.trim().is_empty() {
        return Err(
            AppError::BadRequest("slug and displayName required".to_string())
                .with_request_id(&request_id.0),
        );
    }

    let pricing_tier = payload.pricing_tier.unwrap_or(PricingTier::Free);
    let price_cents = payload.price_cents.unwrap_or(0);
    let is_public = payload.is_public.unwrap_or(true);
    let id = format!("ch_{}", nanoid::nanoid!(12));

    let channel = db::queries::channels::create(
        &state.db,
        &id,
        publisher_id,
        &payload.slug,
        &payload.display_name,
        payload.description.as_deref(),
        payload.category.as_deref(),
        pricing_tier,
        price_cents,
        is_public,
    )
    .await
    .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(ChannelSummaryResponse {
        id: channel.id,
        slug: channel.slug,
        display_name: channel.display_name,
        pricing_tier: channel.pricing_tier,
        price_cents: channel.price_cents,
        subscriber_count: channel.subscriber_count,
        signal_count: channel.signal_count,
    }))
}

async fn list_channels(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> ApiResult<Json<ChannelListResponse>> {
    require_subscriber(&auth, &request_id)?;

    let channels = db::queries::channels::list_marketplace(&state.db)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(ChannelListResponse {
        items: channels
            .into_iter()
            .map(|channel| ChannelListItem {
                id: channel.id,
                slug: channel.slug,
                display_name: channel.display_name,
                pricing_tier: channel.pricing_tier,
                price_cents: channel.price_cents,
            })
            .collect(),
    }))
}

async fn get_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<ChannelDetailResponse>> {
    let channel = db::queries::channels::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("channel not found".to_string()).with_request_id(&request_id.0)
        })?;

    if matches!(channel.status, ChannelStatus::Deleted) {
        return Err(
            AppError::NotFound("channel not found".to_string()).with_request_id(&request_id.0)
        );
    }

    if !channel.is_public
        && (auth.owner_type != db::models::ApiKeyOwner::Publisher
            || channel.publisher_id != auth.owner_id)
    {
        return Err(
            AppError::NotFound("channel not found".to_string()).with_request_id(&request_id.0)
        );
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

async fn update_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateChannelRequest>,
) -> ApiResult<Json<UpdateChannelResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    let channel = db::queries::channels::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("channel not found".to_string()).with_request_id(&request_id.0)
        })?;

    if channel.publisher_id != publisher_id {
        return Err(
            AppError::Forbidden("not channel owner".to_string()).with_request_id(&request_id.0)
        );
    }

    let (id, display_name, updated_at) = db::queries::channels::update(
        &state.db,
        &id,
        payload.display_name.as_deref(),
        payload.description.as_deref(),
        payload.category.as_deref(),
        payload.pricing_tier,
        payload.price_cents,
        payload.is_public,
        payload.status,
    )
    .await
    .map_err(|err| {
        if matches!(err, sqlx::Error::Protocol(_)) {
            AppError::BadRequest("no fields to update".to_string()).with_request_id(&request_id.0)
        } else {
            AppError::Internal.with_request_id(&request_id.0)
        }
    })?;

    Ok(Json(UpdateChannelResponse {
        id,
        display_name,
        updated_at,
    }))
}

async fn delete_channel(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<DeleteChannelResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    let channel = db::queries::channels::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("channel not found".to_string()).with_request_id(&request_id.0)
        })?;

    if channel.publisher_id != publisher_id {
        return Err(
            AppError::Forbidden("not channel owner".to_string()).with_request_id(&request_id.0)
        );
    }

    db::queries::channels::soft_delete(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(DeleteChannelResponse {
        id,
        status: ChannelStatus::Deleted,
    }))
}

async fn channel_stats(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<ChannelStatsResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    let channel = db::queries::channels::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("channel not found".to_string()).with_request_id(&request_id.0)
        })?;

    if channel.publisher_id != publisher_id {
        return Err(
            AppError::Forbidden("not channel owner".to_string()).with_request_id(&request_id.0)
        );
    }

    let totals = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT COALESCE(SUM(delivered_count), 0) as delivered,
               COALESCE(SUM(delivery_count), 0) as total
        FROM signals
        WHERE channel_id = $1
        "#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    let delivery_success_rate = if totals.1 > 0 {
        totals.0 as f64 / totals.1 as f64
    } else {
        0.0
    };

    Ok(Json(ChannelStatsResponse {
        signal_count: channel.signal_count,
        subscriber_count: channel.subscriber_count,
        delivery_success_rate,
    }))
}

fn require_publisher<'a>(
    auth: &'a AuthContext,
    request_id: &RequestId,
) -> Result<&'a str, ApiError> {
    match auth.owner_type {
        db::models::ApiKeyOwner::Publisher => Ok(auth.owner_id.as_str()),
        db::models::ApiKeyOwner::Subscriber => {
            Err(AppError::Forbidden("publisher access required".to_string())
                .with_request_id(&request_id.0))
        }
    }
}

fn require_subscriber<'a>(
    auth: &'a AuthContext,
    request_id: &RequestId,
) -> Result<&'a str, ApiError> {
    match auth.owner_type {
        db::models::ApiKeyOwner::Subscriber => Ok(auth.owner_id.as_str()),
        db::models::ApiKeyOwner::Publisher => Err(AppError::Forbidden(
            "subscriber access required".to_string(),
        )
        .with_request_id(&request_id.0)),
    }
}
