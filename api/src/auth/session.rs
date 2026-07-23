use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use crate::error::ApiError;
use crate::state::AppState;

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub telegram_handle: Option<String>,
    pub usdt_trc20: Option<String>,
    pub usdt_bep20: Option<String>,
    pub usdt_erc20: Option<String>,
}

pub struct CurrentUser(pub User);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get("session")
            .map(|c| c.value().to_string())
            .ok_or(ApiError::Unauthorized)?;
        let user = sqlx::query_as::<_, User>(
            "SELECT u.id, u.email, u.telegram_handle, u.usdt_trc20, u.usdt_bep20, u.usdt_erc20 \
             FROM sessions s JOIN users u ON u.id = s.user_id \
             WHERE s.token_hash = $1 AND s.expires_at > now()",
        )
        .bind(hash_token(&token))
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError::Unauthorized)?;
        Ok(CurrentUser(user))
    }
}
