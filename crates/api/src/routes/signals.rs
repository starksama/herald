use axum::{
    extract::{Path, Query, State},
    routing::post,
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, ApiResult, AppError},
    middleware::auth::AuthContext,
    state::{AppState, RequestId, METRICS},
};
use core::types::DeliveryJob;
use db::models::{ChannelStatus, SignalUrgency};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/v1/channels/:id/signals",
            post(push_signal).get(list_signals),
        )
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushSignalRequest {
    title: String,
    body: String,
    urgency: Option<SignalUrgency>,
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PushSignalResponse {
    id: String,
    channel_id: String,
    status: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListSignalsQuery {
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SignalListItem {
    id: String,
    title: String,
    urgency: SignalUrgency,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSignalsResponse {
    items: Vec<SignalListItem>,
    next_cursor: Option<String>,
}

async fn push_signal(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(channel_id): Path<String>,
    Json(payload): Json<PushSignalRequest>,
) -> ApiResult<Json<PushSignalResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    if payload.title.trim().is_empty() || payload.body.trim().is_empty() {
        return Err(AppError::BadRequest("title and body required".to_string())
            .with_request_id(&request_id.0));
    }

    let channel = db::queries::channels::get_by_id(&state.db, &channel_id)
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

    if !matches!(channel.status, ChannelStatus::Active) {
        return Err(AppError::BadRequest("channel is not active".to_string())
            .with_request_id(&request_id.0));
    }

    let urgency = payload.urgency.unwrap_or(SignalUrgency::Normal);
    let metadata = payload.metadata.unwrap_or_else(|| serde_json::json!({}));
    let id = format!("sig_{}", nanoid::nanoid!(12));

    let signal = db::queries::signals::create(
        &state.db,
        &id,
        &channel_id,
        &payload.title,
        &payload.body,
        urgency.clone(),
        metadata,
    )
    .await
    .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    db::queries::channels::increment_signal_count(&state.db, &channel_id, 1)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    let urgency_label = match urgency {
        SignalUrgency::Low => "low",
        SignalUrgency::Normal => "normal",
        SignalUrgency::High => "high",
        SignalUrgency::Critical => "critical",
    };
    METRICS.record_signal(&channel_id, urgency_label);

    let subs = db::queries::subscriptions::list_active_by_channel(&state.db, &channel_id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    let queue = match urgency {
        SignalUrgency::High | SignalUrgency::Critical => "delivery-high",
        _ => "delivery-normal",
    };

    for sub in subs {
        let job = DeliveryJob {
            signal_id: signal.id.clone(),
            subscription_id: sub.id,
            webhook_id: sub.webhook_id,
            attempt: 0,
        };

        state
            .storage
            .push(queue, job)
            .await
            .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;
    }

    Ok(Json(PushSignalResponse {
        id: signal.id,
        channel_id: signal.channel_id,
        status: "active".to_string(),
        created_at: signal.created_at,
    }))
}

async fn list_signals(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(channel_id): Path<String>,
    Query(query): Query<ListSignalsQuery>,
) -> ApiResult<Json<ListSignalsResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    let channel = db::queries::channels::get_by_id(&state.db, &channel_id)
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

    let limit = query.limit.unwrap_or(50).min(100);
    let signals = db::queries::signals::list_by_channel(
        &state.db,
        &channel_id,
        limit,
        query.cursor.as_deref(),
    )
    .await
    .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    let next_cursor = signals.last().map(|signal| signal.id.clone());

    Ok(Json(ListSignalsResponse {
        items: signals
            .into_iter()
            .map(|signal| SignalListItem {
                id: signal.id,
                title: signal.title,
                urgency: signal.urgency,
                created_at: signal.created_at,
            })
            .collect(),
        next_cursor,
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
