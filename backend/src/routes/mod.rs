pub(crate) mod admin;
pub(crate) mod auth;
pub(crate) mod me;
pub(crate) mod public;

use axum::{Json, Router, response::IntoResponse, routing::get};

use crate::{AppState, schemas::HealthResponse};

#[utoipa::path(
    get,
    path = "/api/health",
    tag = "system",
    responses(
        (status = 200, description = "Backend health status", body = HealthResponse)
    )
)]
pub(crate) async fn health() -> impl IntoResponse {
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
