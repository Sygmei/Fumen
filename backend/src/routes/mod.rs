mod admin;
mod auth;
mod me;
mod public;

use axum::{Json, Router, response::IntoResponse};

use crate::AppState;

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
}

pub fn api_routes(state: AppState) -> Router {
    Router::new()
        .merge(admin::routes(state.clone()))
        .merge(public::routes(state.clone()))
        .merge(auth::routes(state.clone()))
        .merge(me::routes(state.clone()))
        .route("/health", crate::op_get!(state, "/health", health))
        .with_state(state)
}
