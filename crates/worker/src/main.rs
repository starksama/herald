use anyhow::Result;
use core::config::Settings;
use core::types::DeliveryJob;
use core::tunnel::AgentRegistry;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::info;

mod jobs;

#[derive(Clone)]
pub struct WorkerState {
    pub db: sqlx::PgPool,
    pub client: reqwest::Client,
    pub storage: apalis::postgres::PostgresStorage<DeliveryJob>,
    pub tunnel_registry: Arc<AgentRegistry>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let settings = Settings::from_env()?;

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await?;

    let storage =
        apalis::postgres::PostgresStorage::<DeliveryJob>::new(&settings.database_url).await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let state = WorkerState {
        db,
        client,
        storage,
        tunnel_registry: core::tunnel::AGENT_REGISTRY.clone(),
    };

    let handler_state = state.clone();
    let worker_high = apalis::prelude::WorkerBuilder::new("delivery-high")
        .layer(apalis::layers::RetryLayer::new(
            jobs::delivery::retry_policy,
        ))
        .build_fn(move |job: DeliveryJob| {
            let state = handler_state.clone();
            async move { jobs::delivery::handle_delivery_job(&state, job).await }
        });

    let handler_state = state.clone();
    let worker_normal = apalis::prelude::WorkerBuilder::new("delivery-normal")
        .layer(apalis::layers::RetryLayer::new(
            jobs::delivery::retry_policy,
        ))
        .build_fn(move |job: DeliveryJob| {
            let state = handler_state.clone();
            async move { jobs::delivery::handle_delivery_job(&state, job).await }
        });

    info!("worker starting");

    apalis::prelude::Monitor::new()
        .register(worker_high)
        .register(worker_normal)
        .run()
        .await?;

    Ok(())
}
