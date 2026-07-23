use std::sync::Arc;
use std::time::Duration;
use axum::{body::Body, http::{Request, StatusCode}};
use peetopee_api::auth::limiter::RateLimiter;
use peetopee_api::auth::mailer::MemoryMailer;
use peetopee_api::{config::Config, state::AppState};
use sqlx::PgPool;
use tower::ServiceExt;

pub fn test_config() -> Config {
    Config {
        database_url: String::new(),
        base_url: "http://localhost:3000".into(),
        bind_addr: String::new(),
        resend_api_key: None,
        etherscan_api_key: None,
        trongrid_api_key: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn health_returns_ok(pool: PgPool) {
    let mailer = Arc::new(MemoryMailer::default());
    let state = AppState {
        pool,
        config: Arc::new(test_config()),
        mailer,
        limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(900))),
        ip_limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(900))),
    };
    let res = peetopee_api::app(state)
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
