use axum::{
    extract::{Path, State},
    routing::{delete, get},
    Extension, Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{ApiError, ApiResult, AppError},
    middleware::auth::AuthContext,
    state::{AppState, RequestId},
};
use core::auth::{generate_api_key, PUBLISHER_PREFIX};
use db::models::{ApiKeyOwner, ApiKeyStatus};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/publisher/me", get(get_publisher_profile))
        .route(
            "/v1/publisher/api-keys",
            get(list_api_keys).post(create_api_key),
        )
        .route("/v1/publisher/api-keys/{id}", delete(revoke_api_key))
        .with_state(state)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublisherProfileResponse {
    id: String,
    name: String,
    email: String,
    tier: db::models::AccountTier,
    status: db::models::AccountStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiKeyItem {
    id: String,
    prefix: String,
    name: Option<String>,
    status: ApiKeyStatus,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListApiKeysResponse {
    items: Vec<ApiKeyItem>,
}

#[derive(Debug, Deserialize)]
struct CreateApiKeyRequest {
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateApiKeyResponse {
    id: String,
    key: String,
    prefix: String,
}

#[derive(Debug, Serialize)]
struct RevokeApiKeyResponse {
    status: ApiKeyStatus,
}

async fn get_publisher_profile(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> ApiResult<Json<PublisherProfileResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    let publisher = db::queries::publishers::get_by_id(&state.db, publisher_id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?
        .ok_or_else(|| {
            AppError::NotFound("publisher not found".to_string()).with_request_id(&request_id.0)
        })?;

    Ok(Json(PublisherProfileResponse {
        id: publisher.id,
        name: publisher.name,
        email: publisher.email,
        tier: publisher.tier,
        status: publisher.status,
    }))
}

async fn list_api_keys(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
) -> ApiResult<Json<ListApiKeysResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    let keys =
        db::queries::api_keys::list_by_owner(&state.db, ApiKeyOwner::Publisher, publisher_id)
            .await
            .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(ListApiKeysResponse {
        items: keys
            .into_iter()
            .map(|key| ApiKeyItem {
                id: key.id,
                prefix: key.key_prefix,
                name: key.name,
                status: key.status,
                created_at: key.created_at,
            })
            .collect(),
    }))
}

async fn create_api_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> ApiResult<Json<CreateApiKeyResponse>> {
    let publisher_id = require_publisher(&auth, &request_id)?;

    let (raw, hash, prefix) = generate_api_key(PUBLISHER_PREFIX);
    let id = format!("key_{}", nanoid::nanoid!(12));

    db::queries::api_keys::create(
        &state.db,
        &id,
        &hash,
        &prefix,
        ApiKeyOwner::Publisher,
        publisher_id,
        payload.name.as_deref(),
        &[],
    )
    .await
    .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(CreateApiKeyResponse {
        id,
        key: raw,
        prefix,
    }))
}

async fn revoke_api_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Extension(request_id): Extension<RequestId>,
    Path(id): Path<String>,
) -> ApiResult<Json<RevokeApiKeyResponse>> {
    let _publisher_id = require_publisher(&auth, &request_id)?;

    db::queries::api_keys::revoke(&state.db, &id)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id.0))?;

    Ok(Json(RevokeApiKeyResponse {
        status: ApiKeyStatus::Revoked,
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
