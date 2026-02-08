use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, ApiResult, AppError},
    middleware::auth::AuthContext,
    state::{AppState, RequestId},
};
use db::models::{ApiKeyOwner, SubscriptionStatus};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/v1/subscriptions",
            post(create_subscription).get(list_subscriptions),
        )
        .route("/v1/subscriptions/:id", delete(delete_subscription))
        .route("/v1/subscriber/me", get(get_subscriber_profile))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSubscriptionRequest {
    channel_id: String,
    webhook_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateSubscriptionResponse {
    id: String,
    status: SubscriptionStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubscriptionItem {
    id: String,
    channel_id: String,
    webhook_id: String,
    status: SubscriptionStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSubscriptionsResponse {
    items: Vec<SubscriptionItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSubscriptionResponse {
    id: String,
    status: SubscriptionStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubscriberProfileResponse {
    id: String,
    name: String,
    email: String,
    tier: db::models::AccountTier,
    status: db::models::AccountStatus,
}

async fn create_subscription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> ApiResult<Json<CreateSubscriptionResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let channel = db::queries::channels::get_by_id(&state.db, &payload.channel_id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("channel not found".to_string()).with_request_id(&request_id.0)
        })?;

    if !channel.is_public {
        return Err(AppError::BadRequest("channel is not public".to_string())
            .with_request_id(&request_id.0));
    }
    if !matches!(channel.status, db::models::ChannelStatus::Active) {
        return Err(AppError::BadRequest("channel is not active".to_string())
            .with_request_id(&request_id.0));
    }

    let webhook = db::queries::webhooks::get_by_id(&state.db, &payload.webhook_id)
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

    let id = format!("sub_{}", nanoid::nanoid!(12));
    let subscription = db::queries::subscriptions::create(
        &state.db,
        &id,
        subscriber_id,
        &payload.channel_id,
        &payload.webhook_id,
    )
    .await
    .map_err(|err| {
        if let sqlx::Error::Database(db_err) = &err {
            if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) {
                return AppError::BadRequest("already subscribed".to_string())
                    .with_request_id(&request_id.0);
            }
        }
        AppError::Internal.with_request_id(&request_id.0)
    })?;

    db::queries::channels::increment_subscriber_count(&state.db, &payload.channel_id, 1)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(CreateSubscriptionResponse {
        id: subscription.id,
        status: subscription.status,
    }))
}

async fn list_subscriptions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> ApiResult<Json<ListSubscriptionsResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let subs = db::queries::subscriptions::list_by_subscriber(&state.db, subscriber_id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(ListSubscriptionsResponse {
        items: subs
            .into_iter()
            .map(|sub| SubscriptionItem {
                id: sub.id,
                channel_id: sub.channel_id,
                webhook_id: sub.webhook_id,
                status: sub.status,
            })
            .collect(),
    }))
}

async fn delete_subscription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<DeleteSubscriptionResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let subscription = db::queries::subscriptions::get_by_id(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("subscription not found".to_string()).with_request_id(&request_id.0)
        })?;

    if subscription.subscriber_id != subscriber_id {
        return Err(AppError::Forbidden("not subscription owner".to_string())
            .with_request_id(&request_id.0));
    }

    db::queries::subscriptions::update_status(&state.db, &id, SubscriptionStatus::Canceled)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    db::queries::channels::increment_subscriber_count(&state.db, &subscription.channel_id, -1)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(DeleteSubscriptionResponse {
        id,
        status: SubscriptionStatus::Canceled,
    }))
}

async fn get_subscriber_profile(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> ApiResult<Json<SubscriberProfileResponse>> {
    let subscriber_id = require_subscriber(&auth, &request_id)?;

    let subscriber = db::queries::subscribers::get_by_id(&state.db, subscriber_id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("subscriber not found".to_string()).with_request_id(&request_id.0)
        })?;

    Ok(Json(SubscriberProfileResponse {
        id: subscriber.id,
        name: subscriber.name,
        email: subscriber.email,
        tier: subscriber.tier,
        status: subscriber.status,
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
