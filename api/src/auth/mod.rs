pub mod limiter;
pub mod mailer;
pub mod session;

use axum::{extract::{Query, State}, http::{HeaderMap, StatusCode}, Json, response::Redirect};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use uuid::Uuid;
use crate::error::ApiError;
use crate::state::AppState;
use session::{hash_token, new_token, CurrentUser};

#[derive(Deserialize)]
pub struct RequestLinkBody {
    pub email: String,
}

/// Best-effort client IP extraction, trusting proxy headers in order of preference.
/// `cf-connecting-ip` (Cloudflare) wins, then the first hop of `x-forwarded-for`,
/// falling back to "unknown" so the rate limiter still has a stable bucket to use.
fn client_ip(headers: &HeaderMap) -> String {
    if let Some(v) = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
        return v.trim().to_string();
    }
    if let Some(v) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = v.split(',').next() {
            let first = first.trim();
            if !first.is_empty() {
                return first.to_string();
            }
        }
    }
    "unknown".to_string()
}

pub async fn request_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RequestLinkBody>,
) -> Result<StatusCode, ApiError> {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 3 || email.len() > 254 {
        return Err(ApiError::BadRequest("invalid email".into()));
    }
    let ip = client_ip(&headers);
    // Silently accept over-limit requests: no user enumeration, no spam.
    // Both per-email and per-IP caps must pass; either failing denies the request.
    let email_ok = state.limiter.check(&email);
    let ip_ok = state.ip_limiter.check(&ip);
    if !email_ok || !ip_ok {
        return Ok(StatusCode::OK);
    }
    let token = new_token();
    sqlx::query(
        "INSERT INTO login_tokens (email, token_hash, expires_at) \
         VALUES ($1, $2, now() + interval '15 minutes')",
    )
    .bind(&email)
    .bind(hash_token(&token))
    .execute(&state.pool)
    .await?;
    let link = format!("{}/api/auth/verify?token={}", state.config.base_url, token);
    if let Err(e) = state.mailer.send_magic_link(&email, &link).await {
        tracing::error!("failed to send magic link to {email}: {e}");
    }
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct VerifyQuery {
    pub token: String,
}

pub async fn verify(
    State(state): State<AppState>,
    Query(q): Query<VerifyQuery>,
    jar: CookieJar,
) -> Result<(CookieJar, Redirect), ApiError> {
    let base = &state.config.base_url;
    let row: Option<(String,)> = sqlx::query_as(
        "UPDATE login_tokens SET used_at = now() \
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now() \
         RETURNING email",
    )
    .bind(hash_token(&q.token))
    .fetch_optional(&state.pool)
    .await?;
    let Some((email,)) = row else {
        return Ok((jar, Redirect::to(&format!("{base}/login?error=expired"))));
    };
    let (user_id, telegram): (Uuid, Option<String>) = sqlx::query_as(
        "INSERT INTO users (email) VALUES ($1) \
         ON CONFLICT (email) DO UPDATE SET email = EXCLUDED.email \
         RETURNING id, telegram_handle",
    )
    .bind(&email)
    .fetch_one(&state.pool)
    .await?;
    let token = new_token();
    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, expires_at) \
         VALUES ($1, $2, now() + interval '30 days')",
    )
    .bind(user_id)
    .bind(hash_token(&token))
    .execute(&state.pool)
    .await?;
    let cookie = Cookie::build(("session", token))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(30))
        .build();
    let dest = if telegram.is_none() { format!("{base}/onboarding") } else { format!("{base}/orders") };
    Ok((jar.add(cookie), Redirect::to(&dest)))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    _user: CurrentUser,
) -> Result<(CookieJar, StatusCode), ApiError> {
    if let Some(c) = jar.get("session") {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(hash_token(c.value()))
            .execute(&state.pool)
            .await?;
    }
    Ok((jar.remove(Cookie::build(("session", "")).path("/").build()), StatusCode::OK))
}
