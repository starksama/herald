pub mod admin;
pub mod channels;
pub mod health;
pub mod publisher;
pub mod signals;
pub mod subscriptions;
pub mod webhooks;

use axum::Router;

use crate::state::AppState;
use crate::tunnel;

pub fn v1_router(state: AppState) -> Router {
    Router::new()
        .merge(tunnel::router(state.clone()))
        .merge(channels::router(state.clone()))
        .merge(signals::router(state.clone()))
        .merge(subscriptions::router(state.clone()))
        .merge(webhooks::router(state.clone()))
        .merge(publisher::router(state.clone()))
        .merge(admin::router(state))
}

pub fn health_router(state: AppState) -> Router {
    health::router(state)
}
