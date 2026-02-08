use axum::Router;

use crate::state::AppState;

pub mod channels;
pub mod health;
pub mod signals;
pub mod subscriptions;
pub mod webhooks;

pub fn v1_router(state: AppState) -> Router {
    Router::new()
        .merge(channels::router(state.clone()))
        .merge(signals::router(state.clone()))
        .merge(subscriptions::router(state.clone()))
        .merge(webhooks::router(state))
}

pub fn health_router(state: AppState) -> Router {
    health::router(state)
}
