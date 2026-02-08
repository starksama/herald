use axum::{
    body::Body,
    extract::State,
    http::{header::AUTHORIZATION, Request},
    middleware::Next,
    response::Response,
};

use crate::{
    error::{ApiError, AppError},
    state::AppState,
    state::RequestId,
};
use core::auth::hash_api_key;
use db::models::{AccountTier, ApiKeyOwner};

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub owner_type: ApiKeyOwner,
    pub owner_id: String,
    pub tier: AccountTier,
    pub key_id: String,
}

pub async fn api_key_auth(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    if req.uri().path() == "/v1/tunnel" {
        return Ok(next.run(req).await);
    }

    let request_id = req
        .extensions()
        .get::<RequestId>()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| "unknown".to_string());

    let auth = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if token.is_empty() {
        return Err(AppError::Unauthorized.with_request_id(&request_id));
    }

    let hash = hash_api_key(token);
    let api_key = db::queries::api_keys::get_by_hash(&state.db, &hash)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id))?
        .ok_or_else(|| AppError::Unauthorized.with_request_id(&request_id))?;

    let tier = match api_key.owner_type {
        ApiKeyOwner::Publisher => {
            let publisher = db::queries::publishers::get_by_id(&state.db, &api_key.owner_id)
                .await
                .map_err(|_| AppError::Internal.with_request_id(&request_id))?
                .ok_or_else(|| AppError::Unauthorized.with_request_id(&request_id))?;
            publisher.tier
        }
        ApiKeyOwner::Subscriber => {
            let subscriber = db::queries::subscribers::get_by_id(&state.db, &api_key.owner_id)
                .await
                .map_err(|_| AppError::Internal.with_request_id(&request_id))?
                .ok_or_else(|| AppError::Unauthorized.with_request_id(&request_id))?;
            subscriber.tier
        }
    };

    let _ = db::queries::api_keys::touch_last_used(&state.db, &api_key.id).await;

    let ctx = AuthContext {
        owner_type: api_key.owner_type,
        owner_id: api_key.owner_id,
        tier,
        key_id: api_key.id,
    };

    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}
