use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::state::{AppState, METRICS};

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn metrics() -> String {
    METRICS.gather()
}
