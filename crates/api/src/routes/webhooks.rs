use axum::{
    extract::{Path, Query, State},
    routing::{get, patch, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, ApiResult, AppError},
    middleware::auth::AuthContext,
    state::{AppState, RequestId},
};
use db::models::{ApiKeyOwner, DeliveryStatus, WebhookStatus};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/webhooks", post(create_webhook).get(list_webhooks))
        .route(
            "/v1/webhooks/:id",
            patch(update_webhook).delete(delete_webhook),
        )
        .route("/v1/webhooks/:id/deliveries", get(list_deliveries))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWebhookRequest {
    name: String,
    url: String,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateWebhookResponse {
    id: String,
    status: WebhookStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebhookItem {
    id: String,
    name: String,
    url: String,
    status: WebhookStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListWebhooksResponse {
    items: Vec<WebhookItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWebhookRequest {
    name: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateWebhookResponse {
    id: String,
    status: WebhookStatus,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteWebhookResponse {
    id: String,
    status: WebhookStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListDeliveriesQuery {
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryItem {
    id: String,
    status: DeliveryStatus,
    attempt: i32,
    status_code: Option<i32>,
    latency_ms: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListDeliveriesResponse {
    items: Vec<DeliveryItem>,
    next_cursor: Option<String>,
}

async fn create_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Json(payload): Json<CreateWebhookRequest>,
) -> ApiResult<Json<CreateWebhookResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    validate_webhook_url(&payload.url, &state.settings.herald_env)
        .map_err(|msg| AppError::BadRequest(msg).with_request_id(&request_id.0))?;

    let id = format!("wh_{}", nanoid::nanoid!(12));
    let webhook = db::queries::webhooks::create(
        &state.db,
        &id,
        subscriber_id,
        &payload.url,
        &payload.name,
        payload.token.as_deref(),
    )
    .await
    .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(CreateWebhookResponse {
        id: webhook.id,
        status: webhook.status,
    }))
}

async fn list_webhooks(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> ApiResult<Json<ListWebhooksResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let hooks = db::queries::webhooks::list_by_subscriber(&state.db, subscriber_id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(ListWebhooksResponse {
        items: hooks
            .into_iter()
            .map(|hook| WebhookItem {
                id: hook.id,
                name: hook.name,
                url: hook.url,
                status: hook.status,
            })
            .collect(),
    }))
}

async fn update_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWebhookRequest>,
) -> ApiResult<Json<UpdateWebhookResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let webhook = db::queries::webhooks::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("webhook not found".to_string()).with_request_id(&request_id.0)
        })?;

    if webhook.subscriber_id != subscriber_id {
        return Err(
            AppError::Forbidden("not webhook owner".to_string()).with_request_id(&request_id.0)
        );
    }

    if let Some(url) = payload.url.as_deref() {
        validate_webhook_url(url, &state.settings.herald_env)
            .map_err(|msg| AppError::BadRequest(msg).with_request_id(&request_id.0))?;
    }

    let (id, status, updated_at) = db::queries::webhooks::update(
        &state.db,
        &id,
        payload.name.as_deref(),
        payload.url.as_deref(),
        None,
    )
    .await
    .map_err(|err| {
        if matches!(err, sqlx::Error::Protocol(_)) {
            AppError::BadRequest("no fields to update".to_string()).with_request_id(&request_id.0)
        } else {
            AppError::Internal.with_request_id(&request_id.0)
        }
    })?;

    Ok(Json(UpdateWebhookResponse {
        id,
        status,
        updated_at,
    }))
}

async fn delete_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<DeleteWebhookResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let webhook = db::queries::webhooks::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("webhook not found".to_string()).with_request_id(&request_id.0)
        })?;

    if webhook.subscriber_id != subscriber_id {
        return Err(
            AppError::Forbidden("not webhook owner".to_string()).with_request_id(&request_id.0)
        );
    }

    let (id, status, _updated_at) =
        db::queries::webhooks::update(&state.db, &id, None, None, Some(WebhookStatus::Disabled))
            .await
            .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(DeleteWebhookResponse { id, status }))
}

async fn list_deliveries(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
    Query(query): Query<ListDeliveriesQuery>,
) -> ApiResult<Json<ListDeliveriesResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let webhook = db::queries::webhooks::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("webhook not found".to_string()).with_request_id(&request_id.0)
        })?;

    if webhook.subscriber_id != subscriber_id {
        return Err(
            AppError::Forbidden("not webhook owner".to_string()).with_request_id(&request_id.0)
        );
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let deliveries =
        db::queries::deliveries::list_by_webhook(&state.db, &id, limit, query.cursor.as_deref())
            .await
            .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    let next_cursor = deliveries.last().map(|delivery| delivery.id.clone());

    Ok(Json(ListDeliveriesResponse {
        items: deliveries
            .into_iter()
            .map(|delivery| DeliveryItem {
                id: delivery.id,
                status: delivery.status,
                attempt: delivery.attempt,
                status_code: delivery.status_code,
                latency_ms: delivery.latency_ms,
            })
            .collect(),
        next_cursor,
    }))
}

fn require_subscriber<'a>(
    auth: &'a AuthContext,
    request_id: &RequestId,
) -> Result<&'a str, ApiError> {
    match auth.owner_type {
        ApiKeyOwner::Subscriber => Ok(auth.owner_id.as_str()),
        ApiKeyOwner::Publisher => Err(AppError::Forbidden(
            "subscriber access required".to_string(),
        )
        .with_request_id(&request_id.0)),
    }
}

fn validate_webhook_url(url: &str, env: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("webhook url must be https".to_string());
    }

    if env == "prod" {
        let lowered = url.to_lowercase();
        if lowered.contains("localhost")
            || lowered.contains("127.0.0.1")
            || lowered.contains("0.0.0.0")
        {
            return Err("webhook url must not target localhost in prod".to_string());
        }
    }

    Ok(())
}
