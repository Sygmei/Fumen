mod admin;
mod auth;
mod me;
mod public;

use axum::{Json, Router, response::IntoResponse, routing::get};

use crate::AppState;

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
}

pub fn api_routes(state: AppState) -> Router {
    Router::new()
        .merge(admin::routes())
        .merge(public::routes())
        .merge(auth::routes())
        .merge(me::routes())
        .route("/health", get(health))
        .with_state(state)
}
