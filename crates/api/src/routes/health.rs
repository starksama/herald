use axum::{routing::get, Json, Router};
use serde_json::json;

use crate::state::AppState;

pub fn router(_state: AppState) -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}
