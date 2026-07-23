#![allow(dead_code)]

use axum::body::Body;
use axum::http::Request;
use axum::Router;
use std::sync::Arc;
use std::time::Duration;
use peetopee_api::auth::limiter::RateLimiter;
use peetopee_api::auth::mailer::MemoryMailer;
use peetopee_api::{config::Config, state::AppState};
use sqlx::PgPool;
use tower::ServiceExt;

pub fn test_state(pool: PgPool) -> (AppState, Arc<MemoryMailer>) {
    let mailer = Arc::new(MemoryMailer::default());
    let config = Config {
        database_url: String::new(),
        base_url: "http://localhost:3000".into(),
        bind_addr: String::new(),
        resend_api_key: None,
        etherscan_api_key: None,
        trongrid_api_key: None,
    };
    let state = AppState {
        pool,
        config: Arc::new(config),
        mailer: mailer.clone(),
        limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(900))),
        ip_limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(900))),
    };
    (state, mailer)
}

/// Pull the token query param out of a captured magic link.
pub fn extract_token(link: &str) -> String {
    link.split("token=").nth(1).expect("link has token").to_string()
}

/// Full magic-link login; returns the `Cookie` header value ("session=...").
pub async fn login(app: &Router, mailer: &MemoryMailer, email: &str) -> String {
    let res = app.clone()
        .oneshot(
            Request::post("/api/auth/request-link")
                .header("content-type", "application/json")
                .body(Body::from(format!("{{\"email\":\"{email}\"}}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(res.status().is_success());
    let link = mailer.sent.lock().unwrap().last().unwrap().1.clone();
    let token = extract_token(&link);
    let res = app.clone()
        .oneshot(Request::get(format!("/api/auth/verify?token={token}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let set_cookie = res.headers().get("set-cookie").expect("set-cookie").to_str().unwrap();
    set_cookie.split(';').next().unwrap().to_string()
}
