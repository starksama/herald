use axum::{
    extract::{Path, State},
    routing::{delete, get, patch, post},
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
        .route("/v1/webhooks", post(register_webhook).get(list_webhooks))
        .route("/v1/webhooks/:id", patch(update_webhook).delete(delete_webhook))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterWebhookRequest {
    name: String,
    url: String,
    token: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct RegisterWebhookResponse {
    id: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebhookListResponse {
    items: Vec<WebhookListItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebhookListItem {
    id: String,
    name: String,
    url: String,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWebhookRequest {
    name: Option<String>,
    url: Option<String>,
    token: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWebhookResponse {
    id: String,
    status: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteWebhookResponse {
    id: String,
    status: String,
}

#[derive(Debug, sqlx::FromRow)]
struct WebhookRow {
    id: String,
    subscriber_id: String,
    name: String,
    url: String,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct UpdateWebhookRow {
    id: String,
    status: String,
    updated_at: DateTime<Utc>,
}

pub async fn register_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(payload): Json<RegisterWebhookRequest>,
) -> ApiResult<Json<RegisterWebhookResponse>> {
    let subscriber_id = require_subscriber(&auth)?;

    validate_url(&payload.url, &state.herald_env)?;

    if payload.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name required".to_string()));
    }

    let id = format!("wh_{}", nanoid::nanoid!(12));

    let record = sqlx::query_as::<_, RegisterWebhookResponse>(
        r#"
        INSERT INTO webhooks (id, subscriber_id, url, name, token)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, status::text as status
        "#,
    )
    .bind(&id)
    .bind(subscriber_id)
    .bind(&payload.url)
    .bind(&payload.name)
    .bind(payload.token)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(record))
}

pub async fn list_webhooks(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
) -> ApiResult<Json<WebhookListResponse>> {
    let subscriber_id = require_subscriber(&auth)?;

    let items = sqlx::query_as::<_, WebhookRow>(
        r#"
        SELECT id, subscriber_id, name, url, status::text as status, created_at
        FROM webhooks
        WHERE subscriber_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(subscriber_id)
    .fetch_all(&state.db)
    .await?;

    let items = items
        .into_iter()
        .map(|row| WebhookListItem {
            id: row.id,
            name: row.name,
            url: row.url,
            status: row.status,
            created_at: row.created_at,
        })
        .collect();

    Ok(Json(WebhookListResponse { items }))
}

pub async fn update_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWebhookRequest>,
) -> ApiResult<Json<UpdateWebhookResponse>> {
    let subscriber_id = require_subscriber(&auth)?;

    let existing = sqlx::query_as::<_, WebhookRow>(
        r#"
        SELECT id, subscriber_id, name, url, status::text as status, created_at
        FROM webhooks
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let existing = match existing {
        Some(existing) => existing,
        None => return Err(ApiError::NotFound("webhook not found".to_string())),
    };

    if existing.subscriber_id != subscriber_id {
        return Err(ApiError::Forbidden("not webhook owner".to_string()));
    }

    if let Some(url) = payload.url.as_deref() {
        validate_url(url, &state.herald_env)?;
    }

    let mut qb = QueryBuilder::new("UPDATE webhooks SET ");
    let mut set = qb.separated(", ");
    let mut updated = false;

    if let Some(name) = payload.name {
        set.push("name = ").push_bind(name);
        updated = true;
    }
    if let Some(url) = payload.url {
        set.push("url = ").push_bind(url);
        updated = true;
    }
    if let Some(token) = payload.token {
        set.push("token = ").push_bind(token);
        updated = true;
    }
    if let Some(status) = payload.status {
        set.push("status = ").push_bind(status).push("::webhook_status");
        updated = true;
    }

    if !updated {
        return Err(ApiError::BadRequest("no fields to update".to_string()));
    }

    set.push("updated_at = now()");
    qb.push(" WHERE id = ").push_bind(&id);
    qb.push(" RETURNING id, status::text as status, updated_at");

    let record = qb
        .build_query_as::<UpdateWebhookRow>()
        .fetch_one(&state.db)
        .await?;

    Ok(Json(UpdateWebhookResponse {
        id: record.id,
        status: record.status,
        updated_at: record.updated_at,
    }))
}

pub async fn delete_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> ApiResult<Json<DeleteWebhookResponse>> {
    let subscriber_id = require_subscriber(&auth)?;

    let existing = sqlx::query_as::<_, WebhookRow>(
        r#"
        SELECT id, subscriber_id, name, url, status::text as status, created_at
        FROM webhooks
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let existing = match existing {
        Some(existing) => existing,
        None => return Err(ApiError::NotFound("webhook not found".to_string())),
    };

    if existing.subscriber_id != subscriber_id {
        return Err(ApiError::Forbidden("not webhook owner".to_string()));
    }

    sqlx::query(
        r#"
        UPDATE webhooks
        SET status = 'disabled', updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(&id)
    .execute(&state.db)
    .await?;

    Ok(Json(DeleteWebhookResponse {
        id,
        status: "disabled".to_string(),
    }))
}

fn require_subscriber(auth: &AuthContext) -> ApiResult<&str> {
    match auth.owner_type {
        OwnerType::Subscriber => Ok(auth.owner_id.as_str()),
        OwnerType::Publisher => Err(ApiError::Forbidden("subscriber access required".to_string())),
    }
}

fn validate_url(url: &str, env: &str) -> ApiResult<()> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(ApiError::BadRequest("url must start with http:// or https://".to_string()));
    }
    if env == "production" && (url.contains("localhost") || url.contains("127.0.0.1")) {
        return Err(ApiError::BadRequest("url cannot target localhost in production".to_string()));
    }
    Ok(())
}
