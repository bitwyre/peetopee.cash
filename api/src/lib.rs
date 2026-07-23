pub mod auth;
pub mod config;
pub mod error;
pub mod orders;
pub mod state;
pub mod users;
pub mod watcher;

use axum::{routing::{get, post}, Json, Router};
use state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/request-link", post(auth::request_link))
        .route("/api/auth/verify", get(auth::verify))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/me", get(users::get_me).patch(users::update_me))
        .route("/api/orders", post(orders::create))
        .route("/api/orders/mine", get(orders::list_mine))
        .route("/api/orders/open", get(orders::list_open))
        .route("/api/orders/{id}", get(orders::get_detail))
        .route("/api/orders/{id}/accept", post(orders::transitions::accept))
        .route("/api/orders/{id}/request-payment", post(orders::transitions::request_payment))
        .route("/api/orders/{id}/confirm-cash", post(orders::transitions::confirm_cash))
        .route("/api/orders/{id}/cancel", post(orders::transitions::cancel))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
