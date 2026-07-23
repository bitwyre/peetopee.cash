pub mod model;
pub mod transitions;

use axum::extract::{Path, State};
use axum::Json;
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;
use crate::auth::session::CurrentUser;
use crate::error::ApiError;
use crate::state::AppState;
use model::{CourierUsdt, Order, OrderDetail, CURRENCIES, ORDER_COLUMNS};

#[derive(Deserialize)]
pub struct CreateOrder {
    pub fiat_currency: String,
    pub fiat_amount: Decimal,
    pub usdt_amount: Decimal,
    pub address_text: String,
    pub lat: f64,
    pub lng: f64,
}

/// Coarsen an order for non-party viewers: ~500m grid, no street address.
fn redact_location(mut order: Order) -> Order {
    order.address_text = String::new();
    order.lat = (order.lat * 200.0).round() / 200.0;
    order.lng = (order.lng * 200.0).round() / 200.0;
    order
}

pub async fn create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateOrder>,
) -> Result<Json<Order>, ApiError> {
    if !CURRENCIES.contains(&body.fiat_currency.as_str()) {
        return Err(ApiError::BadRequest("unsupported currency".into()));
    }
    if body.fiat_amount <= Decimal::ZERO || body.usdt_amount <= Decimal::ZERO {
        return Err(ApiError::BadRequest("amounts must be positive".into()));
    }
    let addr = body.address_text.trim();
    if addr.is_empty() || addr.chars().count() > 500 {
        return Err(ApiError::BadRequest("address_text must be 1-500 chars".into()));
    }
    if !(-90.0..=90.0).contains(&body.lat) || !(-180.0..=180.0).contains(&body.lng) {
        return Err(ApiError::BadRequest("invalid coordinates".into()));
    }
    let order = sqlx::query_as::<_, Order>(&format!(
        "INSERT INTO orders (customer_id, fiat_currency, fiat_amount, usdt_amount, address_text, lat, lng) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING {ORDER_COLUMNS}"
    ))
    .bind(user.id)
    .bind(&body.fiat_currency)
    .bind(body.fiat_amount)
    .bind(body.usdt_amount)
    .bind(addr)
    .bind(body.lat)
    .bind(body.lng)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(order))
}

pub async fn list_mine(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<Order>>, ApiError> {
    let orders = sqlx::query_as::<_, Order>(&format!(
        "SELECT {ORDER_COLUMNS} FROM orders \
         WHERE customer_id = $1 OR courier_id = $1 ORDER BY created_at DESC"
    ))
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(orders))
}

pub async fn list_open(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Vec<Order>>, ApiError> {
    let orders = sqlx::query_as::<_, Order>(&format!(
        "SELECT {ORDER_COLUMNS} FROM orders \
         WHERE status = 'OPEN' AND customer_id <> $1 ORDER BY created_at DESC"
    ))
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;
    let orders = orders.into_iter().map(redact_location).collect();
    Ok(Json(orders))
}

pub async fn get_detail(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<OrderDetail>, ApiError> {
    let order = sqlx::query_as::<_, Order>(&format!(
        "SELECT {ORDER_COLUMNS} FROM orders WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;

    let is_customer = order.customer_id == user.id;
    let is_courier = order.courier_id == Some(user.id);
    let is_party = is_customer || is_courier;
    if !is_party && order.status != "OPEN" {
        return Err(ApiError::NotFound); // don't leak existence
    }

    let order = if is_party { order } else { redact_location(order) };

    let mut detail = OrderDetail {
        order, is_customer, is_courier,
        customer_telegram: None, courier_telegram: None, courier_usdt: None,
    };

    if is_party {
        if let Some(courier_id) = detail.order.courier_id {
            let (ct,): (Option<String>,) =
                sqlx::query_as("SELECT telegram_handle FROM users WHERE id = $1")
                    .bind(detail.order.customer_id).fetch_one(&state.pool).await?;
            detail.customer_telegram = ct;
            let (kt, trc20, bep20, erc20): (Option<String>, Option<String>, Option<String>, Option<String>) =
                sqlx::query_as("SELECT telegram_handle, usdt_trc20, usdt_bep20, usdt_erc20 FROM users WHERE id = $1")
                    .bind(courier_id).fetch_one(&state.pool).await?;
            detail.courier_telegram = kt;
            if matches!(detail.order.status.as_str(), "AWAITING_PAYMENT" | "PAID" | "COMPLETED") {
                detail.courier_usdt = Some(CourierUsdt { trc20, bep20, erc20 });
            }
        }
    }
    Ok(Json(detail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn sample(lat: f64, lng: f64) -> Order {
        Order {
            id: Uuid::nil(),
            customer_id: Uuid::nil(),
            courier_id: None,
            fiat_currency: "IDR".into(),
            fiat_amount: Decimal::ZERO,
            usdt_amount: Decimal::ZERO,
            address_text: "Jl. Sunset Road 99, Kuta".into(),
            lat,
            lng,
            status: "OPEN".into(),
            payment_network: None,
            payment_txid: None,
            payment_requested_at: None,
            paid_at: None,
            created_at: Utc::now(),
            accepted_at: None,
            completed_at: None,
            cancelled_at: None,
        }
    }

    #[test]
    fn redact_snaps_to_500m_grid_and_blanks_address() {
        let r = redact_location(sample(-8.6705, 115.2126));
        assert_eq!(r.lat, -8.67);
        assert_eq!(r.lng, 115.215);
        assert_eq!(r.address_text, "");
    }

    #[test]
    fn redact_grid_is_finer_than_one_decimal() {
        // A point ~600m east must land on a different grid cell than the origin.
        let a = redact_location(sample(0.0, 0.0));
        let b = redact_location(sample(0.0, 0.0055));
        assert_ne!(a.lng, b.lng);
    }
}
