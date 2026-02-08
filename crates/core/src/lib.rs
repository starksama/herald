pub mod types {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Ids {
        pub id: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Publisher {
        pub id: String,
        pub name: String,
        pub email: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Subscriber {
        pub id: String,
        pub name: String,
        pub email: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Channel {
        pub id: String,
        pub publisher_id: String,
        pub slug: String,
        pub display_name: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Signal {
        pub id: String,
        pub channel_id: String,
        pub title: String,
        pub body: String,
        pub created_at: DateTime<Utc>,
    }
}

pub mod auth {
    use hmac::{Hmac, Mac};
    use nanoid::nanoid;
    use sha2::{Digest, Sha256};

    pub fn generate_api_key(prefix: &str) -> (String, String, String) {
        let raw = format!("{}{}", prefix, nanoid!(24));
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let key_prefix = raw.chars().take(12).collect::<String>();
        (raw, hash, key_prefix)
    }

    pub fn sign_payload(secret: &str, timestamp: i64, body: &str) -> String {
        let data = format!("{}.{}", timestamp, body);
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("HMAC key");
        mac.update(data.as_bytes());
        format!("sha256={:x}", mac.finalize().into_bytes())
    }
}

pub mod config {
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
}
