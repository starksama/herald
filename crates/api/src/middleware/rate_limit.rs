use crate::{
    error::{ApiError, AppError},
    middleware::auth::AuthContext,
    state::{AppState, RequestId},
};
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use db::models::AccountTier;

pub async fn rate_limit(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let request_id = req
        .extensions()
        .get::<RequestId>()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let auth = req
        .extensions()
        .get::<AuthContext>()
        .cloned()
        .ok_or_else(|| AppError::Unauthorized.with_request_id(&request_id))?;

    let capacity = match auth.tier {
        AccountTier::Free => state.settings.rate_limit_free,
        AccountTier::Pro => state.settings.rate_limit_pro,
        AccountTier::Enterprise => state.settings.rate_limit_ent,
    };

    let mut conn = state
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id))?;

    let allowed = allow_request(&mut conn, &auth.key_id, capacity, capacity)
        .await
        .map_err(|_| AppError::Internal.with_request_id(&request_id))?;

    if !allowed {
        return Err(AppError::RateLimited.with_request_id(&request_id));
    }

    Ok(next.run(req).await)
}

async fn allow_request(
    conn: &mut redis::aio::MultiplexedConnection,
    key: &str,
    capacity: u32,
    refill_per_min: u32,
) -> redis::RedisResult<bool> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let bucket_key = format!("rl:{}", key);

    let script = r#"
local bucket = KEYS[1]
local now = tonumber(ARGV[1])
local capacity = tonumber(ARGV[2])
local refill = tonumber(ARGV[3])

local data = redis.call('HMGET', bucket, 'tokens', 'ts')
local tokens = tonumber(data[1]) or capacity
local ts = tonumber(data[2]) or now

local delta = math.max(0, now - ts)
local new_tokens = math.min(capacity, tokens + (delta * refill / 60))

if new_tokens < 1 then
  redis.call('HMSET', bucket, 'tokens', new_tokens, 'ts', now)
  redis.call('EXPIRE', bucket, 120)
  return 0
else
  new_tokens = new_tokens - 1
  redis.call('HMSET', bucket, 'tokens', new_tokens, 'ts', now)
  redis.call('EXPIRE', bucket, 120)
  return 1
end
"#;

    let allowed: i32 = redis::Script::new(script)
        .key(bucket_key)
        .arg(now)
        .arg(capacity)
        .arg(refill_per_min)
        .invoke_async(conn)
        .await?;

    Ok(allowed == 1)
}
