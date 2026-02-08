use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Request},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

use crate::{error::{ApiError, ApiResult}, state::AppState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerType {
    Publisher,
    Subscriber,
}

impl OwnerType {
    fn from_db(value: &str) -> Option<Self> {
        match value {
            "publisher" => Some(OwnerType::Publisher),
            "subscriber" => Some(OwnerType::Subscriber),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub key_id: String,
    pub owner_type: OwnerType,
    pub owner_id: String,
    pub key_prefix: String,
}

pub async fn api_key_auth(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let header_value = req
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or_else(|| ApiError::Unauthorized("missing authorization header".to_string()))?;

    let token = parse_bearer(header_value)?;
    let hash = hash_key(token);

    let record = sqlx::query_as::<_, ApiKeyRecord>(
        r#"
        SELECT id, owner_type::text as owner_type, owner_id, key_prefix, expires_at
        FROM api_keys
        WHERE key_hash = $1 AND status = 'active'
        LIMIT 1
        "#,
    )
    .bind(&hash)
    .fetch_optional(&state.db)
    .await?;

    let record = match record {
        Some(record) => record,
        None => return Err(ApiError::Unauthorized("invalid api key".to_string())),
    };

    if let Some(expires_at) = record.expires_at {
        if expires_at < chrono::Utc::now() {
            return Err(ApiError::Unauthorized("api key expired".to_string()));
        }
    }

    let owner_type = OwnerType::from_db(&record.owner_type)
        .ok_or_else(|| ApiError::Unauthorized("invalid api key owner".to_string()))?;

    sqlx::query(
        r#"
        UPDATE api_keys SET last_used_at = now()
        WHERE id = $1
        "#,
    )
    .bind(&record.id)
    .execute(&state.db)
    .await?;

    req.extensions_mut().insert(AuthContext {
        key_id: record.id,
        owner_type,
        owner_id: record.owner_id,
        key_prefix: record.key_prefix,
    });

    Ok(next.run(req).await)
}

fn parse_bearer(value: &HeaderValue) -> ApiResult<&str> {
    let value = value
        .to_str()
        .map_err(|_| ApiError::Unauthorized("invalid authorization header".to_string()))?;
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or_default();
    if scheme != "Bearer" || token.is_empty() {
        return Err(ApiError::Unauthorized("invalid authorization header".to_string()));
    }
    Ok(token)
}

fn hash_key(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, sqlx::FromRow)]
struct ApiKeyRecord {
    id: String,
    owner_type: String,
    owner_id: String,
    key_prefix: String,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}
