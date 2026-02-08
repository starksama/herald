use axum::{middleware::from_fn_with_state, Router};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

mod error;
mod middleware;
mod routes;
mod state;

use crate::middleware::auth::api_key_auth;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let database_url = std::env::var("HERALD_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("DATABASE_URL or HERALD_DATABASE_URL must be set");
    let redis_url = std::env::var("HERALD_REDIS_URL")
        .or_else(|_| std::env::var("REDIS_URL"))
        .expect("REDIS_URL or HERALD_REDIS_URL must be set");
    let herald_env = std::env::var("HERALD_ENV").unwrap_or_else(|_| "development".to_string());

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    let redis = redis::Client::open(redis_url)?;

    let state = AppState {
        db,
        redis,
        herald_env,
    };

    let v1 = routes::v1_router(state.clone()).layer(from_fn_with_state(state.clone(), api_key_auth));

    let app = Router::new()
        .merge(routes::health_router(state.clone()))
        .merge(v1);

    let addr: SocketAddr = std::env::var("HERALD_API_BIND")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string())
        .parse()?;

    info!(%addr, "starting api");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
