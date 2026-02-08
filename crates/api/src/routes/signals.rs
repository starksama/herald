use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{
    error::{ApiError, ApiResult},
    middleware::auth::{AuthContext, OwnerType},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/channels/{id}/signals", post(push_signal).get(list_signals))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushSignalRequest {
    title: String,
    body: String,
    urgency: Option<String>,
    metadata: Option<JsonValue>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct PushSignalResponse {
    id: String,
    channel_id: String,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSignalsResponse {
    items: Vec<SignalListItem>,
    next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SignalListItem {
    id: String,
    title: String,
    urgency: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChannelOwnerRow {
    id: String,
    publisher_id: String,
    status: String,
    is_public: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct SignalRow {
    id: String,
    title: String,
    urgency: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct SignalCursorRow {
    created_at: DateTime<Utc>,
}

pub async fn push_signal(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(channel_id): Path<String>,
    Json(payload): Json<PushSignalRequest>,
) -> ApiResult<Json<PushSignalResponse>> {
    let publisher_id = require_publisher(&auth)?;

    if payload.title.trim().is_empty() || payload.body.trim().is_empty() {
        return Err(ApiError::BadRequest("title and body required".to_string()));
    }

    let channel = sqlx::query_as::<_, ChannelOwnerRow>(
        r#"
        SELECT id, publisher_id, status::text as status, is_public
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&channel_id)
    .fetch_optional(&state.db)
    .await?;

    let channel = match channel {
        Some(channel) => channel,
        None => return Err(ApiError::NotFound("channel not found".to_string())),
    };

    if channel.publisher_id != publisher_id {
        return Err(ApiError::Forbidden("not channel owner".to_string()));
    }

    if channel.status != "active" {
        return Err(ApiError::BadRequest("channel not active".to_string()));
    }

    let id = format!("sig_{}", nanoid::nanoid!(12));
    let urgency = payload.urgency.unwrap_or_else(|| "normal".to_string());
    let metadata = payload.metadata.unwrap_or_else(|| JsonValue::Object(Default::default()));

    let mut tx = state.db.begin().await?;

    let record = sqlx::query_as::<_, PushSignalResponse>(
        r#"
        INSERT INTO signals (id, channel_id, title, body, urgency, metadata)
        VALUES ($1, $2, $3, $4, $5::signal_urgency, $6)
        RETURNING id, channel_id, status::text as status, created_at
        "#,
    )
    .bind(&id)
    .bind(&channel_id)
    .bind(&payload.title)
    .bind(&payload.body)
    .bind(&urgency)
    .bind(metadata)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE channels
        SET signal_count = signal_count + 1, updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(&channel_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(record))
}

pub async fn list_signals(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(channel_id): Path<String>,
    Query(pagination): Query<Pagination>,
) -> ApiResult<Json<ListSignalsResponse>> {
    let channel = sqlx::query_as::<_, ChannelOwnerRow>(
        r#"
        SELECT id, publisher_id, status::text as status, is_public
        FROM channels
        WHERE id = $1
        "#,
    )
    .bind(&channel_id)
    .fetch_optional(&state.db)
    .await?;

    let channel = match channel {
        Some(channel) => channel,
        None => return Err(ApiError::NotFound("channel not found".to_string())),
    };

    if channel.status == "deleted" {
        return Err(ApiError::NotFound("channel not found".to_string()));
    }

    let allowed = match auth.owner_type {
        OwnerType::Publisher => channel.publisher_id == auth.owner_id,
        OwnerType::Subscriber => channel.is_public,
    };

    if !allowed {
        return Err(ApiError::Forbidden("access denied".to_string()));
    }

    let limit = pagination.limit.unwrap_or(50).clamp(1, 100);

    let mut cursor_time: Option<DateTime<Utc>> = None;
    if let Some(cursor) = pagination.cursor.as_deref() {
        let cursor_row = sqlx::query_as::<_, SignalCursorRow>(
            r#"
            SELECT created_at
            FROM signals
            WHERE id = $1 AND channel_id = $2
            "#,
        )
        .bind(cursor)
        .bind(&channel_id)
        .fetch_optional(&state.db)
        .await?;

        let cursor_row = cursor_row.ok_or_else(|| ApiError::BadRequest("invalid cursor".to_string()))?;
        cursor_time = Some(cursor_row.created_at);
    }

    let signals = if let Some(cursor_time) = cursor_time {
        sqlx::query_as::<_, SignalRow>(
            r#"
            SELECT id, title, urgency::text as urgency, created_at
            FROM signals
            WHERE channel_id = $1 AND created_at < $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(&channel_id)
        .bind(cursor_time)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, SignalRow>(
            r#"
            SELECT id, title, urgency::text as urgency, created_at
            FROM signals
            WHERE channel_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(&channel_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await?
    };

    let next_cursor = signals.last().map(|row| row.id.clone());

    let items = signals
        .into_iter()
        .map(|row| SignalListItem {
            id: row.id,
            title: row.title,
            urgency: row.urgency,
            created_at: row.created_at,
        })
        .collect();

    Ok(Json(ListSignalsResponse { items, next_cursor }))
}

fn require_publisher(auth: &AuthContext) -> ApiResult<&str> {
    match auth.owner_type {
        OwnerType::Publisher => Ok(auth.owner_id.as_str()),
        OwnerType::Subscriber => Err(ApiError::Forbidden("publisher access required".to_string())),
    }
}
