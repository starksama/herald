use axum::{middleware::from_fn, middleware::from_fn_with_state, Router};
use core::config::Settings;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

mod error;
mod middleware;
mod routes;
mod state;

use crate::middleware::auth::api_key_auth;
use crate::middleware::metrics::metrics;
use crate::middleware::rate_limit::rate_limit;
use crate::middleware::request_id::request_id;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let settings = Settings::from_env()?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&settings.database_url)
        .await?;

    let redis = redis::Client::open(settings.redis_url.clone())?;
    let storage = apalis::postgres::PostgresStorage::new(&settings.database_url).await?;

    let state = AppState {
        db,
        redis,
        storage,
        settings: settings.clone(),
    };

    let v1 = routes::v1_router(state.clone())
        .layer(from_fn_with_state(state.clone(), rate_limit))
        .layer(from_fn_with_state(state.clone(), api_key_auth))
        .layer(from_fn(metrics))
        .layer(from_fn(request_id));

    let app = Router::new()
        .merge(routes::health_router(state.clone()))
        .merge(v1)
        .layer(axum::extract::DefaultBodyLimit::max(1_048_576));

    let addr: SocketAddr = settings.api_bind.parse()?;
    info!(%addr, "starting api");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
