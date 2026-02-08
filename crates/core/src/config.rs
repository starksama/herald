use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub database_url: String,
    pub redis_url: String,
    pub herald_env: String,
    pub api_bind: String,
    pub worker_concurrency: usize,
    pub hmac_secret: String,
    pub rate_limit_free: u32,
    pub rate_limit_pro: u32,
    pub rate_limit_ent: u32,
}

impl Settings {
    pub fn from_env() -> Result<Self, std::env::VarError> {
        let database_url =
            std::env::var("DATABASE_URL").or_else(|_| std::env::var("HERALD_DATABASE_URL"))?;
        let redis_url =
            std::env::var("REDIS_URL").or_else(|_| std::env::var("HERALD_REDIS_URL"))?;
        let herald_env = std::env::var("HERALD_ENV").unwrap_or_else(|_| "dev".to_string());
        let api_bind =
            std::env::var("HERALD_API_BIND").unwrap_or_else(|_| "0.0.0.0:3000".to_string());
        let worker_concurrency = std::env::var("HERALD_WORKER_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);
        let hmac_secret =
            std::env::var("HERALD_HMAC_SECRET").or_else(|_| std::env::var("HMAC_SECRET"))?;
        let rate_limit_free = std::env::var("HERALD_RATE_LIMIT_FREE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
        let rate_limit_pro = std::env::var("HERALD_RATE_LIMIT_PRO")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(600);
        let rate_limit_ent = std::env::var("HERALD_RATE_LIMIT_ENT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(6000);

        Ok(Self {
            database_url,
            redis_url,
            herald_env,
            api_bind,
            worker_concurrency,
            hmac_secret,
            rate_limit_free,
            rate_limit_pro,
            rate_limit_ent,
        })
    }
}
