pub mod models {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use sqlx::FromRow;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Channel {
        pub id: String,
        pub publisher_id: String,
        pub slug: String,
        pub display_name: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Signal {
        pub id: String,
        pub channel_id: String,
        pub title: String,
        pub body: String,
        pub created_at: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Subscription {
        pub id: String,
        pub subscriber_id: String,
        pub channel_id: String,
        pub webhook_id: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }
}

pub mod queries {
    use super::models;
    use sqlx::PgPool;

    pub async fn get_channel_by_id(
        pool: &PgPool,
        id: &str,
    ) -> Result<Option<models::Channel>, sqlx::Error> {
        let channel = sqlx::query_as::<_, models::Channel>(
            r#"
            SELECT id, publisher_id, slug, display_name, created_at, updated_at
            FROM channels
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(channel)
    }

    pub async fn list_signals_for_channel(
        pool: &PgPool,
        channel_id: &str,
        limit: i64,
    ) -> Result<Vec<models::Signal>, sqlx::Error> {
        let signals = sqlx::query_as::<_, models::Signal>(
            r#"
            SELECT id, channel_id, title, body, created_at
            FROM signals
            WHERE channel_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(channel_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(signals)
    }
}
