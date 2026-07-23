use std::sync::LazyLock;
use axum::{extract::State, Json};
use regex::Regex;
use serde::Deserialize;
use crate::auth::session::{CurrentUser, User};
use crate::error::ApiError;
use crate::state::AppState;

static TG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_]{5,32}$").unwrap());
static TRC20_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^T[1-9A-HJ-NP-Za-km-z]{33}$").unwrap());
static EVM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^0x[0-9a-fA-F]{40}$").unwrap());

pub async fn get_me(CurrentUser(user): CurrentUser) -> Json<User> {
    Json(user)
}

fn double_option<'de, D>(d: D) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(d).map(Some)
}

#[derive(Deserialize)]
pub struct UpdateMe {
    pub telegram_handle: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub usdt_trc20: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub usdt_bep20: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub usdt_erc20: Option<Option<String>>,
}

fn clean_addr(v: Option<String>, re: &Regex, name: &str) -> Result<Option<String>, ApiError> {
    match v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) if re.is_match(&s) => Ok(Some(s)),
        Some(_) => Err(ApiError::BadRequest(format!("invalid {name} address"))),
    }
}

pub async fn update_me(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UpdateMe>,
) -> Result<Json<User>, ApiError> {
    let tg = body.telegram_handle
        .map(|s| {
            let trimmed = s.trim();
            trimmed.strip_prefix('@').unwrap_or(trimmed).to_string()
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::BadRequest("telegram_handle is required".into()))?;
    if !TG_RE.is_match(&tg) {
        return Err(ApiError::BadRequest("invalid telegram handle (5-32 letters, digits, underscore)".into()));
    }
    let trc20 = match body.usdt_trc20 {
        None => user.usdt_trc20.clone(),
        Some(inner) => clean_addr(inner, &TRC20_RE, "TRC20")?,
    };
    let bep20 = match body.usdt_bep20 {
        None => user.usdt_bep20.clone(),
        Some(inner) => clean_addr(inner, &EVM_RE, "BEP20")?,
    };
    let erc20 = match body.usdt_erc20 {
        None => user.usdt_erc20.clone(),
        Some(inner) => clean_addr(inner, &EVM_RE, "ERC20")?,
    };
    let updated = sqlx::query_as::<_, User>(
        "UPDATE users SET telegram_handle = $1, usdt_trc20 = $2, usdt_bep20 = $3, usdt_erc20 = $4 \
         WHERE id = $5 \
         RETURNING id, email, telegram_handle, usdt_trc20, usdt_bep20, usdt_erc20",
    )
    .bind(&tg)
    .bind(&trc20)
    .bind(&bep20)
    .bind(&erc20)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(updated))
}
