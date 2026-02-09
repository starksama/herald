use axum::{
    extract::{Path, State},
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    error::{ApiError, ApiResult, AppError},
    middleware::auth::AuthContext,
    state::{AppState, RequestId},
};
use core::types::DeliveryJob;
use db::models::{ApiKeyOwner, DeliveryStatus};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/admin/dlq", get(list_dlq))
        .route("/v1/admin/dlq/{id}/retry", post(retry_dlq))
        .route("/v1/admin/signals/{id}", get(get_signal_admin))
        .with_state(state)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DlqItem {
    id: String,
    signal_id: String,
    subscription_id: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DlqListResponse {
    items: Vec<DlqItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DlqRetryResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSignalResponse {
    signal: AdminSignal,
    deliveries: Vec<AdminDelivery>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSignal {
    id: String,
    title: String,
    urgency: db::models::SignalUrgency,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminDelivery {
    id: String,
    status: DeliveryStatus,
    attempt: i32,
    status_code: Option<i32>,
}

async fn list_dlq(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> ApiResult<Json<DlqListResponse>> {
    require_publisher(&auth, &request_id)?;

    let entries = db::queries::dead_letter_queue::list_unresolved(&state.db)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(DlqListResponse {
        items: entries
            .into_iter()
            .map(|entry| DlqItem {
                id: entry.id,
                signal_id: entry.signal_id,
                subscription_id: entry.subscription_id,
                created_at: entry.created_at,
            })
            .collect(),
    }))
}

async fn retry_dlq(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<DlqRetryResponse>> {
    require_publisher(&auth, &request_id)?;

    let entry = db::queries::dead_letter_queue::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("dlq entry not found".to_string()).with_request_id(&request_id.0)
        })?;

    let delivery = db::queries::deliveries::get_by_id(&state.db, &entry.delivery_id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("delivery not found".to_string()).with_request_id(&request_id.0)
        })?;

    let job = DeliveryJob {
        signal_id: entry.signal_id,
        subscription_id: entry.subscription_id,
        webhook_id: delivery.webhook_id,
        attempt: 0,
    };

    state
        .storage
        .push("delivery-normal", job)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    db::queries::dead_letter_queue::resolve(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(DlqRetryResponse { status: "queued" }))
}

async fn get_signal_admin(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<AdminSignalResponse>> {
    require_publisher(&auth, &request_id)?;

    let signal = db::queries::signals::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("signal not found".to_string()).with_request_id(&request_id.0)
        })?;

    let deliveries = db::queries::deliveries::list_by_signal(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(AdminSignalResponse {
        signal: AdminSignal {
            id: signal.id,
            title: signal.title,
            urgency: signal.urgency,
            created_at: signal.created_at,
        },
        deliveries: deliveries
            .into_iter()
            .map(|delivery| AdminDelivery {
                id: delivery.id,
                status: delivery.status,
                attempt: delivery.attempt,
                status_code: delivery.status_code,
            })
            .collect(),
    }))
}

fn require_publisher<'a>(
    auth: &'a AuthContext,
    request_id: &RequestId,
) -> Result<&'a str, ApiError> {
    match auth.owner_type {
        ApiKeyOwner::Publisher => Ok(auth.owner_id.as_str()),
        ApiKeyOwner::Subscriber => {
            Err(AppError::Forbidden("publisher access required".to_string())
                .with_request_id(&request_id.0))
        }
    }
}
