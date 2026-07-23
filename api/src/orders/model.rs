use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

pub const CURRENCIES: [&str; 8] = ["IDR", "AED", "EUR", "GBP", "RUB", "INR", "USD", "CAD"];

/// Column list matching Order's FromRow order; use in every SELECT/RETURNING.
pub const ORDER_COLUMNS: &str =
    "id, customer_id, courier_id, fiat_currency, fiat_amount, usdt_amount, address_text, \
     lat, lng, status, payment_network, payment_txid, payment_requested_at, paid_at, \
     created_at, accepted_at, completed_at, cancelled_at";

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Order {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub courier_id: Option<Uuid>,
    pub fiat_currency: String,
    pub fiat_amount: Decimal,
    pub usdt_amount: Decimal,
    pub address_text: String,
    pub lat: f64,
    pub lng: f64,
    pub status: String,
    pub payment_network: Option<String>,
    pub payment_txid: Option<String>,
    pub payment_requested_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct CourierUsdt {
    pub trc20: Option<String>,
    pub bep20: Option<String>,
    pub erc20: Option<String>,
}

#[derive(Serialize)]
pub struct OrderDetail {
    #[serde(flatten)]
    pub order: Order,
    pub is_customer: bool,
    pub is_courier: bool,
    pub customer_telegram: Option<String>,
    pub courier_telegram: Option<String>,
    pub courier_usdt: Option<CourierUsdt>,
}
