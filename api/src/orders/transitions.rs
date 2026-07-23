use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;
use crate::auth::session::CurrentUser;
use crate::error::ApiError;
use crate::state::AppState;
use super::model::{Order, ORDER_COLUMNS};

pub async fn accept(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Order>, ApiError> {
    if user.usdt_trc20.is_none() && user.usdt_bep20.is_none() && user.usdt_erc20.is_none() {
        return Err(ApiError::BadRequest(
            "add at least one USDT address in settings before accepting orders".into(),
        ));
    }
    sqlx::query_as::<_, Order>(&format!(
        "UPDATE orders SET status = 'ACCEPTED', courier_id = $1, accepted_at = now() \
         WHERE id = $2 AND status = 'OPEN' AND customer_id <> $1 RETURNING {ORDER_COLUMNS}"
    ))
    .bind(user.id).bind(id)
    .fetch_optional(&state.pool).await?
    .map(Json)
    .ok_or_else(|| ApiError::Conflict("order is not open, or it is your own".into()))
}

pub async fn request_payment(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Order>, ApiError> {
    sqlx::query_as::<_, Order>(&format!(
        "UPDATE orders SET status = 'AWAITING_PAYMENT', payment_requested_at = now() \
         WHERE id = $1 AND status = 'ACCEPTED' AND courier_id = $2 RETURNING {ORDER_COLUMNS}"
    ))
    .bind(id).bind(user.id)
    .fetch_optional(&state.pool).await?
    .map(Json)
    .ok_or_else(|| ApiError::Conflict("only the courier can request payment on an accepted order".into()))
}

pub async fn confirm_cash(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Order>, ApiError> {
    sqlx::query_as::<_, Order>(&format!(
        "UPDATE orders SET status = 'COMPLETED', completed_at = now() \
         WHERE id = $1 AND status = 'PAID' AND customer_id = $2 RETURNING {ORDER_COLUMNS}"
    ))
    .bind(id).bind(user.id)
    .fetch_optional(&state.pool).await?
    .map(Json)
    .ok_or_else(|| ApiError::Conflict("only the customer can confirm a paid order".into()))
}

pub async fn cancel(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Order>, ApiError> {
    // Guards, matching the spec's cancel rules:
    //  OPEN: customer only. ACCEPTED: either party.
    //  AWAITING_PAYMENT: either party, but only 2h+ after payment_requested_at.
    sqlx::query_as::<_, Order>(&format!(
        "UPDATE orders SET status = 'CANCELLED', cancelled_at = now() \
         WHERE id = $1 AND ( \
             (status = 'OPEN' AND customer_id = $2) \
          OR (status = 'ACCEPTED' AND (customer_id = $2 OR courier_id = $2)) \
          OR (status = 'AWAITING_PAYMENT' AND (customer_id = $2 OR courier_id = $2) \
              AND payment_requested_at < now() - interval '2 hours') \
         ) RETURNING {ORDER_COLUMNS}"
    ))
    .bind(id).bind(user.id)
    .fetch_optional(&state.pool).await?
    .map(Json)
    .ok_or_else(|| ApiError::Conflict("order cannot be cancelled by you right now".into()))
}
