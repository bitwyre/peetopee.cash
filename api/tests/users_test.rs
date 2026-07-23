mod common;

use axum::{body::Body, http::{Request, StatusCode}};
use sqlx::PgPool;
use tower::ServiceExt;

fn patch_me(cookie: &str, body: serde_json::Value) -> Request<Body> {
    Request::patch("/api/me")
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn me_requires_auth(pool: PgPool) {
    let (state, _) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let res = app.oneshot(Request::get("/api/me").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_me_sets_profile(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let cookie = common::login(&app, &mailer, "f@example.com").await;
    let res = app.clone().oneshot(patch_me(&cookie, serde_json::json!({
        "telegram_handle": "@dendi_s",
        "usdt_trc20": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
        "usdt_erc20": "0xdAC17F958D2ee523a2206206994597C13D831ec7"
    }))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["telegram_handle"], "dendi_s"); // @ stripped
    assert_eq!(v["usdt_trc20"], "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
    assert_eq!(v["usdt_erc20"], "0xdAC17F958D2ee523a2206206994597C13D831ec7");
    assert_eq!(v["usdt_bep20"], serde_json::Value::Null);
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_me_omitted_address_is_unchanged_but_empty_clears(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let cookie = common::login(&app, &mailer, "h@example.com").await;

    // First PATCH sets all three addresses.
    let res = app.clone().oneshot(patch_me(&cookie, serde_json::json!({
        "telegram_handle": "@dendi_s",
        "usdt_trc20": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
        "usdt_bep20": "0xdAC17F958D2ee523a2206206994597C13D831ec7",
        "usdt_erc20": "0xdAC17F958D2ee523a2206206994597C13D831ec7"
    }))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Second PATCH omits usdt_trc20/usdt_bep20 entirely and should leave them
    // unchanged, while explicitly clearing usdt_erc20 with an empty string.
    let res = app.clone().oneshot(patch_me(&cookie, serde_json::json!({
        "telegram_handle": "dendi_s",
        "usdt_erc20": ""
    }))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["usdt_trc20"], "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"); // unchanged
    assert_eq!(v["usdt_bep20"], "0xdAC17F958D2ee523a2206206994597C13D831ec7"); // unchanged
    assert_eq!(v["usdt_erc20"], serde_json::Value::Null); // cleared by empty string

    // Third PATCH explicitly sets usdt_trc20 to null, which should also clear it.
    let res = app.clone().oneshot(patch_me(&cookie, serde_json::json!({
        "telegram_handle": "dendi_s",
        "usdt_trc20": null
    }))).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["usdt_trc20"], serde_json::Value::Null); // cleared by null
    assert_eq!(v["usdt_bep20"], "0xdAC17F958D2ee523a2206206994597C13D831ec7"); // still unchanged
}

#[sqlx::test(migrations = "./migrations")]
async fn patch_me_rejects_bad_inputs(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let cookie = common::login(&app, &mailer, "g@example.com").await;
    for bad in [
        serde_json::json!({"telegram_handle": "ab"}),
        serde_json::json!({"telegram_handle": "valid_name", "usdt_trc20": "0xdAC17F958D2ee523a2206206994597C13D831ec7"}),
        serde_json::json!({"telegram_handle": "valid_name", "usdt_erc20": "not-an-address"}),
        serde_json::json!({}),
    ] {
        let res = app.clone().oneshot(patch_me(&cookie, bad)).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
}
