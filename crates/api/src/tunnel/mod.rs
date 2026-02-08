use axum::{routing::get, Router};

use crate::state::AppState;

pub mod protocol;
pub mod registry;
pub mod server;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/tunnel", get(server::tunnel_ws))
        .with_state(state)
}
