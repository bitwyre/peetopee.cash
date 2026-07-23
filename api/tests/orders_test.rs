mod common;

use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

pub fn json_req(method: &str, uri: &str, cookie: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder().method(method).uri(uri)
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(body.to_string())).unwrap()
}

pub fn get_req(uri: &str, cookie: &str) -> Request<Body> {
    Request::get(uri).header("cookie", cookie).body(Body::empty()).unwrap()
}

pub async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let b = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&b).unwrap()
}

pub fn order_body() -> serde_json::Value {
    serde_json::json!({
        "fiat_currency": "IDR",
        "fiat_amount": "1500000",
        "usdt_amount": "92.5",
        "address_text": "Jl. Sunset Road 99, Kuta",
        "lat": -8.6705,
        "lng": 115.2126
    })
}

#[sqlx::test(migrations = "./migrations")]
async fn create_and_list_orders(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let cookie = common::login(&app, &mailer, "cust@example.com").await;
    let res = app.clone().oneshot(json_req("POST", "/api/orders", &cookie, order_body())).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let order = body_json(res).await;
    assert_eq!(order["status"], "OPEN");
    let usdt_str = order["usdt_amount"].as_str().unwrap();
    assert!(usdt_str.starts_with("92.5"), "expected 92.5 but got {}", usdt_str);

    let res = app.clone().oneshot(get_req("/api/orders/mine", &cookie)).await.unwrap();
    let mine = body_json(res).await;
    let mine = mine.as_array().unwrap();
    assert_eq!(mine.len(), 1);
    // customer's own view still has the full address
    assert_eq!(mine[0]["address_text"], "Jl. Sunset Road 99, Kuta");

    // customer's own order not on their courier board
    let res = app.clone().oneshot(get_req("/api/orders/open", &cookie)).await.unwrap();
    assert_eq!(body_json(res).await.as_array().unwrap().len(), 0);

    // but visible on another user's board, with address redacted
    let cookie2 = common::login(&app, &mailer, "cour@example.com").await;
    let res = app.clone().oneshot(get_req("/api/orders/open", &cookie2)).await.unwrap();
    let open = body_json(res).await;
    let open = open.as_array().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0]["address_text"], "");
}

#[sqlx::test(migrations = "./migrations")]
async fn create_rejects_invalid(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let cookie = common::login(&app, &mailer, "h@example.com").await;
    let mut bad_currency = order_body(); bad_currency["fiat_currency"] = "JPY".into();
    let mut bad_amount = order_body(); bad_amount["usdt_amount"] = "0".into();
    let mut bad_lat = order_body(); bad_lat["lat"] = serde_json::json!(123.0);
    for bad in [bad_currency, bad_amount, bad_lat] {
        let res = app.clone().oneshot(json_req("POST", "/api/orders", &cookie, bad)).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    // Address validation: 501 characters should be rejected
    let mut addr_too_long = order_body();
    addr_too_long["address_text"] = "д".repeat(501).into();
    let res = app.clone().oneshot(json_req("POST", "/api/orders", &cookie, addr_too_long)).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST, "501-char address should be rejected");

    // Address validation: 300 Cyrillic characters (~600 bytes) should be accepted
    let mut addr_cyrillic = order_body();
    addr_cyrillic["address_text"] = "д".repeat(300).into();
    let res = app.clone().oneshot(json_req("POST", "/api/orders", &cookie, addr_cyrillic)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "300-char Cyrillic address should be accepted");
}

#[sqlx::test(migrations = "./migrations")]
async fn open_order_detail_hides_private_fields(pool: PgPool) {
    let (state, mailer) = common::test_state(pool);
    let app = peetopee_api::app(state);
    let cookie = common::login(&app, &mailer, "i@example.com").await;
    let res = app.clone().oneshot(json_req("POST", "/api/orders", &cookie, order_body())).await.unwrap();
    let id = body_json(res).await["id"].as_str().unwrap().to_string();
    let cookie2 = common::login(&app, &mailer, "j@example.com").await;
    let res = app.clone().oneshot(get_req(&format!("/api/orders/{id}"), &cookie2)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let v = body_json(res).await;
    assert!(v["customer_telegram"].is_null());
    assert!(v["courier_usdt"].is_null());
    assert_eq!(v["is_customer"], false);
    assert_eq!(v["address_text"], "");
    assert_eq!(v["lat"], -8.67);
    assert_eq!(v["lng"], 115.21);
}
