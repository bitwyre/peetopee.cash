mod common;

use axum::{body::Body, http::{Request, StatusCode}};
use sqlx::PgPool;
use tower::ServiceExt;

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn json_post_from_ip(uri: &str, body: serde_json::Value, ip: &str) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn request_link_sends_email_with_token(pool: PgPool) {
    let (state, mailer) = common::test_state(pool.clone());
    let app = peetopee_api::app(state);
    let res = app
        .oneshot(json_post("/api/auth/request-link", serde_json::json!({"email": "A@Example.com"})))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let sent = mailer.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "a@example.com");
    assert!(sent[0].1.contains("/api/auth/verify?token="));
    drop(sent);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM login_tokens WHERE email = 'a@example.com'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn request_link_rejects_bad_email(pool: PgPool) {
    let (state, _) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let res = app
        .oneshot(json_post("/api/auth/request-link", serde_json::json!({"email": "nope"})))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn request_link_rate_limits_silently(pool: PgPool) {
    use std::sync::Arc;
    use std::time::Duration;
    let (mut state, mailer) = common::test_state(pool);
    state.limiter = Arc::new(peetopee_api::auth::limiter::RateLimiter::new(2, Duration::from_secs(900)));
    let app = peetopee_api::app(state);
    for _ in 0..3 {
        let res = app.clone()
            .oneshot(json_post("/api/auth/request-link", serde_json::json!({"email": "b@example.com"})))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
    assert_eq!(mailer.sent.lock().unwrap().len(), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn request_link_rate_limits_per_ip_across_different_emails(pool: PgPool) {
    use std::sync::Arc;
    use std::time::Duration;
    let (mut state, mailer) = common::test_state(pool);
    state.ip_limiter = Arc::new(peetopee_api::auth::limiter::RateLimiter::new(2, Duration::from_secs(900)));
    let app = peetopee_api::app(state);
    for email in ["f1@example.com", "f2@example.com", "f3@example.com"] {
        let res = app.clone()
            .oneshot(json_post_from_ip(
                "/api/auth/request-link",
                serde_json::json!({"email": email}),
                "203.0.113.9",
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
    // Same IP, 3 distinct emails, cap of 2: only the first two should have triggered an email send.
    assert_eq!(mailer.sent.lock().unwrap().len(), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_creates_user_session_and_redirects_new_user_to_onboarding(pool: PgPool) {
    let (state, mailer) = common::test_state(pool.clone());
    let app = peetopee_api::app(state);
    app.clone()
        .oneshot(json_post("/api/auth/request-link", serde_json::json!({"email": "c@example.com"})))
        .await.unwrap();
    let link = mailer.sent.lock().unwrap()[0].1.clone();
    let token = common::extract_token(&link);
    let res = app.clone()
        .oneshot(Request::get(format!("/api/auth/verify?token={token}")).body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    let loc = res.headers().get("location").unwrap().to_str().unwrap();
    assert!(loc.ends_with("/onboarding"));
    assert!(res.headers().get("set-cookie").is_some());
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE email = 'c@example.com'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn verify_rejects_reused_token(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    app.clone()
        .oneshot(json_post("/api/auth/request-link", serde_json::json!({"email": "d@example.com"})))
        .await.unwrap();
    let token = common::extract_token(&mailer.sent.lock().unwrap()[0].1.clone());
    for expected_suffix in ["/onboarding", "/login?error=expired"] {
        let res = app.clone()
            .oneshot(Request::get(format!("/api/auth/verify?token={token}")).body(Body::empty()).unwrap())
            .await.unwrap();
        let loc = res.headers().get("location").unwrap().to_str().unwrap();
        assert!(loc.ends_with(expected_suffix), "got {loc}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn logout_kills_session(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let cookie = common::login(&app, &mailer, "e@example.com").await;
    let res = app.clone()
        .oneshot(Request::post("/api/auth/logout").header("cookie", &cookie).body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let res = app.clone()
        .oneshot(Request::post("/api/auth/logout").header("cookie", &cookie).body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
