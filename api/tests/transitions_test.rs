mod common;

use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

fn post_req(uri: &str, cookie: &str) -> Request<Body> {
    Request::post(uri).header("cookie", cookie).body(Body::empty()).unwrap()
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let b = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&b).unwrap()
}

async fn patch_profile(app: &axum::Router, cookie: &str, body: serde_json::Value) {
    let res = app.clone().oneshot(
        Request::patch("/api/me")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(body.to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

/// Creates customer + courier (both have TRC20 addresses) and one OPEN order.
/// Returns (customer_cookie, courier_cookie, order_id).
async fn setup(app: &axum::Router, mailer: &peetopee_api::auth::mailer::MemoryMailer) -> (String, String, String) {
    let cust = common::login(app, mailer, "cust@example.com").await;
    let cour = common::login(app, mailer, "cour@example.com").await;
    patch_profile(app, &cust, serde_json::json!({
        "telegram_handle": "customer_one",
        "usdt_trc20": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"
    })).await;
    patch_profile(app, &cour, serde_json::json!({
        "telegram_handle": "courier_one",
        "usdt_trc20": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"
    })).await;
    let res = app.clone().oneshot(
        Request::post("/api/orders")
            .header("content-type", "application/json")
            .header("cookie", &cust)
            .body(Body::from(serde_json::json!({
                "fiat_currency": "IDR", "fiat_amount": "1500000", "usdt_amount": "92.5",
                "address_text": "Jl. Sunset Road 99", "lat": -8.67, "lng": 115.21
            }).to_string())).unwrap()
    ).await.unwrap();
    let id = body_json(res).await["id"].as_str().unwrap().to_string();
    (cust, cour, id)
}

#[sqlx::test(migrations = "./migrations")]
async fn full_happy_path(pool: PgPool) {
    let (state, mailer) = common::test_state(pool.clone());
    let app = peetopee_api::app(state);
    let (cust, cour, id) = setup(&app, &mailer).await;

    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/accept"), &cour)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["status"], "ACCEPTED");

    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/request-payment"), &cour)).await.unwrap();
    assert_eq!(body_json(res).await["status"], "AWAITING_PAYMENT");

    // watcher normally does this; simulate payment landing
    sqlx::query("UPDATE orders SET status='PAID', payment_network='trc20', payment_txid='abc', paid_at=now() WHERE id = $1::uuid")
        .bind(&id).execute(&pool).await.unwrap();

    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/confirm-cash"), &cust)).await.unwrap();
    assert_eq!(body_json(res).await["status"], "COMPLETED");
}

#[sqlx::test(migrations = "./migrations")]
async fn guards_reject_wrong_actors_and_states(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let (cust, cour, id) = setup(&app, &mailer).await;

    // customer can't accept own order
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/accept"), &cust)).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // courier without USDT address can't accept
    let naked = common::login(&app, &mailer, "naked@example.com").await;
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/accept"), &naked)).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // customer can't request payment; courier can't confirm cash
    app.clone().oneshot(post_req(&format!("/api/orders/{id}/accept"), &cour)).await.unwrap();
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/request-payment"), &cust)).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/confirm-cash"), &cour)).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // second accept fails (already ACCEPTED)
    let cour2 = common::login(&app, &mailer, "cour2@example.com").await;
    patch_profile(&app, &cour2, serde_json::json!({
        "telegram_handle": "courier_two", "usdt_erc20": "0xdAC17F958D2ee523a2206206994597C13D831ec7"
    })).await;
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/accept"), &cour2)).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "./migrations")]
async fn cancel_rules(pool: PgPool) {
    let (state, mailer) = common::test_state(pool.clone());
    let app = peetopee_api::app(state);
    let (cust, cour, id) = setup(&app, &mailer).await;

    // outsider can't cancel an OPEN order
    let outsider = common::login(&app, &mailer, "out@example.com").await;
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/cancel"), &outsider)).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);

    // either party can cancel while ACCEPTED
    app.clone().oneshot(post_req(&format!("/api/orders/{id}/accept"), &cour)).await.unwrap();
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id}/cancel"), &cour)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // AWAITING_PAYMENT: blocked inside 2h, allowed after
    let (cust2, cour2, id2) = {
        let cust2 = cust.clone(); let cour2 = cour.clone();
        let res = app.clone().oneshot(
            Request::post("/api/orders").header("content-type", "application/json").header("cookie", &cust2)
                .body(Body::from(serde_json::json!({
                    "fiat_currency": "USD", "fiat_amount": "100", "usdt_amount": "101",
                    "address_text": "somewhere", "lat": 1.0, "lng": 1.0
                }).to_string())).unwrap()
        ).await.unwrap();
        let id2 = body_json(res).await["id"].as_str().unwrap().to_string();
        (cust2, cour2, id2)
    };
    app.clone().oneshot(post_req(&format!("/api/orders/{id2}/accept"), &cour2)).await.unwrap();
    app.clone().oneshot(post_req(&format!("/api/orders/{id2}/request-payment"), &cour2)).await.unwrap();
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id2}/cancel"), &cust2)).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    sqlx::query("UPDATE orders SET payment_requested_at = now() - interval '3 hours' WHERE id = $1::uuid")
        .bind(&id2).execute(&pool).await.unwrap();
    let res = app.clone().oneshot(post_req(&format!("/api/orders/{id2}/cancel"), &cust2)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
