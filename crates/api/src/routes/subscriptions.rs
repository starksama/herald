use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, ApiResult},
    middleware::auth::{AuthContext, OwnerType},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/subscriptions", post(subscribe).get(list_subscriptions))
        .route("/v1/subscriptions/:id", delete(unsubscribe))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubscribeRequest {
    channel_id: String,
    webhook_id: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct SubscribeResponse {
    id: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubscriptionListResponse {
    items: Vec<SubscriptionListItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubscriptionListItem {
    id: String,
    channel_id: String,
    webhook_id: String,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsubscribeResponse {
    id: String,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ChannelRow {
    id: String,
    status: String,
    is_public: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct WebhookRow {
    id: String,
    subscriber_id: String,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SubscriptionRow {
    id: String,
    subscriber_id: String,
    channel_id: String,
    webhook_id: String,
    status: String,
    created_at: DateTime<Utc>,
}

pub async fn subscribe(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<SubscribeRequest>,
) -> ApiResult<Json<SubscribeResponse>> {
    let subscriber_id = require_subscriber(&auth)?;

    let channel = sqlx::query_as::<_, ChannelRow>(
        r#"
        SELECT id, status::text as status, is_public
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&payload.channel_id)
    .fetch_optional(&state.db)
    .await?;

    let channel = match channel {
        Some(channel) => channel,
        None => return Err(ApiError::NotFound("channel not found".to_string())),
    };

    if channel.status != "active" || !channel.is_public {
        return Err(ApiError::BadRequest("channel not available".to_string()));
    }

    let webhook = sqlx::query_as::<_, WebhookRow>(
        r#"
        SELECT id, subscriber_id, status::text as status
        FROM webhooks
        WHERE id = $1
        "#,
    )
    .bind(&payload.webhook_id)
    .fetch_optional(&state.db)
    .await?;

    let webhook = match webhook {
        Some(webhook) => webhook,
        None => return Err(ApiError::NotFound("webhook not found".to_string())),
    };

    if webhook.subscriber_id != subscriber_id {
        return Err(ApiError::Forbidden("webhook not owned".to_string()));
    }

    if webhook.status != "active" {
        return Err(ApiError::BadRequest("webhook not active".to_string()));
    }

    let id = format!("sub_{}", nanoid::nanoid!(12));
    let mut tx = state.db.begin().await?;

    let record = sqlx::query_as::<_, SubscribeResponse>(
        r#"
        INSERT INTO subscriptions (id, subscriber_id, channel_id, webhook_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, status::text as status
        "#,
    )
    .bind(&id)
    .bind(subscriber_id)
    .bind(&payload.channel_id)
    .bind(&payload.webhook_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE channels
        SET subscriber_count = subscriber_count + 1, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(&payload.channel_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(record))
}

pub async fn list_subscriptions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<SubscriptionListResponse>> {
    let subscriber_id = require_subscriber(&auth)?;

    let items = sqlx::query_as::<_, SubscriptionRow>(
        r#"
        SELECT id, subscriber_id, channel_id, webhook_id, status::text as status, created_at
        FROM subscriptions
        WHERE subscriber_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(subscriber_id)
    .fetch_all(&state.db)
    .await?;

    let items = items
        .into_iter()
        .map(|row| SubscriptionListItem {
            id: row.id,
            channel_id: row.channel_id,
            webhook_id: row.webhook_id,
            status: row.status,
            created_at: row.created_at,
        })
        .collect();

    Ok(Json(SubscriptionListResponse { items }))
}

pub async fn unsubscribe(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> ApiResult<Json<UnsubscribeResponse>> {
    let subscriber_id = require_subscriber(&auth)?;

    let existing = sqlx::query_as::<_, SubscriptionRow>(
        r#"
        SELECT id, subscriber_id, channel_id, webhook_id, status::text as status, created_at
        FROM subscriptions
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let existing = match existing {
        Some(existing) => existing,
        None => return Err(ApiError::NotFound("subscription not found".to_string())),
    };

    if existing.subscriber_id != subscriber_id {
        return Err(ApiError::Forbidden("not subscription owner".to_string()));
    }

    let mut tx = state.db.begin().await?;

    sqlx::query(
        r#"
        UPDATE subscriptions
        SET status = 'canceled', updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE channels
        SET subscriber_count = GREATEST(subscriber_count - 1, 0), updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(&existing.channel_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(UnsubscribeResponse {
        id,
        status: "canceled".to_string(),
    }))
}

fn require_subscriber(auth: &AuthContext) -> ApiResult<&str> {
    match auth.owner_type {
        OwnerType::Subscriber => Ok(auth.owner_id.as_str()),
        OwnerType::Publisher => Err(ApiError::Forbidden("subscriber access required".to_string())),
    }
}
