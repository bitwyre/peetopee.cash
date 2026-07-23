# peetopee.cash Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** P2P cash-delivery webapp: customer orders cash in local currency, courier delivers it, customer pays courier USDT verified on-chain.

**Architecture:** Rust API (Axum + SQLx + Postgres) with a Tokio chain-watcher task; Next.js App Router frontend; Docker Compose (Caddy/web/api/db) on one DigitalOcean droplet. Spec: `docs/superpowers/specs/2026-07-17-peetopee-cash-design.md`.

**Tech Stack:** Rust (axum 0.8, sqlx 0.8 runtime queries — no compile-time query macros), Postgres 16, Next.js 15 + React 19 + TypeScript strict + Tailwind 4, react-leaflet 5, qrcode.react 4, Resend (email), TronGrid + Etherscan V2 (chain data), Caddy w/ Cloudflare DNS-01.

## Global Constraints

- Currencies exactly: `IDR, AED, EUR, GBP, RUB, INR, USD, CAD`
- Order statuses exactly: `OPEN, ACCEPTED, AWAITING_PAYMENT, PAID, COMPLETED, CANCELLED`
- Networks exactly (lowercase): `trc20, bep20, erc20`
- USDT contracts: TRC20 `TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t`; ERC20 `0xdAC17F958D2ee523a2206206994597C13D831ec7` (6 decimals); BEP20 `0x55d398326f99059fF775485246999027B3197955` (18 decimals). Tron USDT has 6 decimals.
- Ports: api `8080`, web `3000`, dev Postgres host port `5433`
- Dev DB URL (also needed as env for Rust tests): `postgres://peetopee:peetopee@localhost:5433/peetopee`
- All API errors are JSON `{"error": "<message>"}`
- API crate is a lib (`peetopee_api`) + thin `main.rs` so integration tests can build the router
- Rust: no `unwrap()` in request handlers (only in `main`/tests); `sqlx::query`/`query_as` runtime binding only
- Frontend: TypeScript strict; amounts arrive as JSON **strings** (rust_decimal serializes Decimal as string)
- Run Rust tests from `api/`: `DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
- Commit after every task; messages `feat: …` / `chore: …`

---

### Task 1: Rust API scaffold + health endpoint

**Files:**
- Create: `.gitignore`, `api/Cargo.toml`, `api/src/main.rs`, `api/src/lib.rs`, `api/src/config.rs`, `api/src/error.rs`, `api/tests/health_test.rs`

**Interfaces:**
- Produces: `peetopee_api::app() -> axum::Router` (Task 2 changes it to `app(state: AppState)`); `config::Config::from_env()`; `error::ApiError` used by all later handlers.

- [ ] **Step 1: Write scaffold files**

`.gitignore` (repo root):
```
/api/target
/web/node_modules
/web/.next
.env
.DS_Store
```

`api/Cargo.toml`:
```toml
[package]
name = "peetopee-api"
version = "0.1.0"
edition = "2021"

[lib]
name = "peetopee_api"
path = "src/lib.rs"

[dependencies]
axum = "0.8"
axum-extra = { version = "0.10", features = ["cookie"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "uuid", "chrono", "rust_decimal", "migrate"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = { version = "1", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
rand = "0.8"
sha2 = "0.10"
hex = "0.4"
regex = "1"
time = "0.3"
async-trait = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

`api/src/config.rs`:
```rust
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub base_url: String,
    pub bind_addr: String,
    pub resend_api_key: Option<String>,
    pub etherscan_api_key: Option<String>,
    pub trongrid_api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL required"),
            base_url: std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into()),
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            resend_api_key: std::env::var("RESEND_API_KEY").ok(),
            etherscan_api_key: std::env::var("ETHERSCAN_API_KEY").ok(),
            trongrid_api_key: std::env::var("TRONGRID_API_KEY").ok(),
        }
    }
}
```

`api/src/error.rs`:
```rust
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::Internal(m) => {
                tracing::error!("internal error: {m}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}
```

`api/src/lib.rs`:
```rust
pub mod config;
pub mod error;

use axum::{routing::get, Json, Router};

pub fn app() -> Router {
    Router::new().route("/api/health", get(health))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
```

`api/src/main.rs`:
```rust
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("listening on {addr}");
    axum::serve(listener, peetopee_api::app()).await.unwrap();
}
```

- [ ] **Step 2: Write the failing/passing test**

`api/tests/health_test.rs`:
```rust
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_ok() {
    let app = peetopee_api::app();
    let res = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```

- [ ] **Step 3: Run tests**

Run: `cd api && cargo test`
Expected: `health_returns_ok ... ok`

- [ ] **Step 4: Commit**

```bash
git add .gitignore api
git commit -m "feat: rust api scaffold with health endpoint"
```

---

### Task 2: Postgres schema, dev DB, AppState

**Files:**
- Create: `compose.dev.yml`, `api/migrations/0001_init.sql`, `api/src/state.rs`
- Modify: `api/src/lib.rs`, `api/src/main.rs`, `api/tests/health_test.rs`
- Test: `api/tests/db_test.rs`

**Interfaces:**
- Produces: `state::AppState { pool: PgPool, config: Arc<Config> }` (Task 3 adds `mailer`, `limiter` fields); `app(state: AppState) -> Router`; DB tables `users`, `login_tokens`, `sessions`, `orders`.

- [ ] **Step 1: Write dev compose + migration + state**

`compose.dev.yml` (repo root):
```yaml
services:
  db:
    image: postgres:16
    environment:
      POSTGRES_USER: peetopee
      POSTGRES_PASSWORD: peetopee
      POSTGRES_DB: peetopee
    ports:
      - "5433:5432"
    volumes:
      - devpg:/var/lib/postgresql/data
volumes:
  devpg:
```

`api/migrations/0001_init.sql`:
```sql
CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL UNIQUE,
  telegram_handle TEXT,
  usdt_trc20 TEXT,
  usdt_bep20 TEXT,
  usdt_erc20 TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE login_tokens (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  expires_at TIMESTAMPTZ NOT NULL,
  used_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE orders (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  customer_id UUID NOT NULL REFERENCES users(id),
  courier_id UUID REFERENCES users(id),
  fiat_currency TEXT NOT NULL CHECK (fiat_currency IN ('IDR','AED','EUR','GBP','RUB','INR','USD','CAD')),
  fiat_amount NUMERIC(18,2) NOT NULL CHECK (fiat_amount > 0),
  usdt_amount NUMERIC(18,6) NOT NULL CHECK (usdt_amount > 0),
  address_text TEXT NOT NULL,
  lat DOUBLE PRECISION NOT NULL,
  lng DOUBLE PRECISION NOT NULL,
  status TEXT NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','ACCEPTED','AWAITING_PAYMENT','PAID','COMPLETED','CANCELLED')),
  payment_network TEXT CHECK (payment_network IN ('trc20','bep20','erc20')),
  payment_txid TEXT UNIQUE,
  payment_requested_at TIMESTAMPTZ,
  paid_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  accepted_at TIMESTAMPTZ,
  completed_at TIMESTAMPTZ,
  cancelled_at TIMESTAMPTZ
);

CREATE INDEX orders_status_idx ON orders(status);
CREATE INDEX orders_customer_idx ON orders(customer_id);
CREATE INDEX orders_courier_idx ON orders(courier_id);
```

`api/src/state.rs`:
```rust
use std::sync::Arc;
use sqlx::PgPool;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
}
```

Modify `api/src/lib.rs` (full new content):
```rust
pub mod config;
pub mod error;
pub mod state;

use axum::{routing::get, Json, Router};
use state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
```

Modify `api/src/main.rs` (full new content):
```rust
use std::sync::Arc;
use peetopee_api::{config::Config, state::AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let config = Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url).await.expect("db connect");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrate");
    let state = AppState { pool, config: Arc::new(config.clone()) };
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await.unwrap();
    tracing::info!("listening on {}", config.bind_addr);
    axum::serve(listener, peetopee_api::app(state)).await.unwrap();
}
```

- [ ] **Step 2: Start dev Postgres**

Run: `docker compose -f compose.dev.yml up -d && sleep 3`

- [ ] **Step 3: Write DB test; update health test**

`api/tests/db_test.rs`:
```rust
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn migrations_create_tables(pool: PgPool) {
    for table in ["users", "login_tokens", "sessions", "orders"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }
}
```

`api/tests/health_test.rs` (full new content):
```rust
use std::sync::Arc;
use axum::{body::Body, http::{Request, StatusCode}};
use peetopee_api::{config::Config, state::AppState};
use sqlx::PgPool;
use tower::ServiceExt;

pub fn test_config() -> Config {
    Config {
        database_url: String::new(),
        base_url: "http://localhost:3000".into(),
        bind_addr: String::new(),
        resend_api_key: None,
        etherscan_api_key: None,
        trongrid_api_key: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn health_returns_ok(pool: PgPool) {
    let state = AppState { pool, config: Arc::new(test_config()) };
    let res = peetopee_api::app(state)
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```

- [ ] **Step 4: Run tests**

Run: `cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
Expected: both tests pass (`#[sqlx::test]` creates throwaway databases via DATABASE_URL).

- [ ] **Step 5: Commit**

```bash
git add compose.dev.yml api
git commit -m "feat: postgres schema, dev db, app state"
```

---

### Task 3: Magic-link request (mailer, rate limiter)

**Files:**
- Create: `api/src/auth/mod.rs`, `api/src/auth/session.rs`, `api/src/auth/mailer.rs`, `api/src/auth/limiter.rs`, `api/tests/common/mod.rs`
- Modify: `api/src/lib.rs`, `api/src/state.rs`, `api/src/main.rs`
- Test: `api/tests/auth_test.rs`

**Interfaces:**
- Consumes: `AppState`, `ApiError`, `Config`.
- Produces: `auth::session::{new_token() -> String, hash_token(&str) -> String}`; `auth::mailer::{Mailer (trait, async fn send_magic_link(&self, to:&str, link:&str) -> Result<(),String>), ResendMailer, LogMailer, MemoryMailer{sent: Mutex<Vec<(String,String)>>}}`; `auth::limiter::RateLimiter::{new(max:u32, window:Duration), check(&self, key:&str)->bool}`; `AppState` gains `mailer: Arc<dyn Mailer>`, `limiter: Arc<RateLimiter>`; route `POST /api/auth/request-link`; test helper `common::{test_state(pool)->(AppState,Arc<MemoryMailer>), extract_token(link:&str)->String}`.

- [ ] **Step 1: Write session token helpers, mailer, limiter**

`api/src/auth/session.rs`:
```rust
use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}
```

`api/src/auth/mailer.rs`:
```rust
use std::sync::Mutex;

#[async_trait::async_trait]
pub trait Mailer: Send + Sync {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String>;
}

pub struct ResendMailer {
    pub api_key: String,
    pub http: reqwest::Client,
}

#[async_trait::async_trait]
impl Mailer for ResendMailer {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String> {
        let body = serde_json::json!({
            "from": "peetopee.cash <login@peetopee.cash>",
            "to": [to],
            "subject": "Your peetopee.cash login link",
            "html": format!(
                "<p>Click to log in to peetopee.cash:</p><p><a href=\"{link}\">{link}</a></p><p>This link expires in 15 minutes.</p>"
            ),
        });
        let res = self.http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("resend returned {}", res.status()));
        }
        Ok(())
    }
}

/// Dev fallback when RESEND_API_KEY is unset: logs the link.
pub struct LogMailer;

#[async_trait::async_trait]
impl Mailer for LogMailer {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String> {
        tracing::info!("magic link for {to}: {link}");
        Ok(())
    }
}

/// Test mailer capturing (to, link) pairs.
#[derive(Default)]
pub struct MemoryMailer {
    pub sent: Mutex<Vec<(String, String)>>,
}

#[async_trait::async_trait]
impl Mailer for MemoryMailer {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<(), String> {
        self.sent.lock().unwrap().push((to.to_string(), link.to_string()));
        Ok(())
    }
}
```

`api/src/auth/limiter.rs`:
```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    max: u32,
    window: Duration,
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self { max, window, hits: Mutex::new(HashMap::new()) }
    }

    /// Returns true if the request is allowed, and records it.
    pub fn check(&self, key: &str) -> bool {
        let mut hits = self.hits.lock().unwrap();
        let now = Instant::now();
        let entry = hits.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < self.window);
        if entry.len() as u32 >= self.max {
            return false;
        }
        entry.push(now);
        true
    }
}
```

- [ ] **Step 2: Write request-link handler and wire state**

`api/src/auth/mod.rs`:
```rust
pub mod limiter;
pub mod mailer;
pub mod session;

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use crate::error::ApiError;
use crate::state::AppState;
use session::{hash_token, new_token};

#[derive(Deserialize)]
pub struct RequestLinkBody {
    pub email: String,
}

pub async fn request_link(
    State(state): State<AppState>,
    Json(body): Json<RequestLinkBody>,
) -> Result<StatusCode, ApiError> {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 3 || email.len() > 254 {
        return Err(ApiError::BadRequest("invalid email".into()));
    }
    // Silently accept over-limit requests: no user enumeration, no spam.
    if !state.limiter.check(&email) {
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
```

Modify `api/src/state.rs` (full new content):
```rust
use std::sync::Arc;
use sqlx::PgPool;
use crate::auth::limiter::RateLimiter;
use crate::auth::mailer::Mailer;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub mailer: Arc<dyn Mailer>,
    pub limiter: Arc<RateLimiter>,
}
```

Modify `api/src/lib.rs` (full new content):
```rust
pub mod auth;
pub mod config;
pub mod error;
pub mod state;

use axum::{routing::{get, post}, Json, Router};
use state::AppState;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/request-link", post(auth::request_link))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
```

Modify `api/src/main.rs` (full new content):
```rust
use std::sync::Arc;
use std::time::Duration;
use peetopee_api::auth::limiter::RateLimiter;
use peetopee_api::auth::mailer::{LogMailer, Mailer, ResendMailer};
use peetopee_api::{config::Config, state::AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let config = Config::from_env();
    let pool = sqlx::PgPool::connect(&config.database_url).await.expect("db connect");
    sqlx::migrate!("./migrations").run(&pool).await.expect("migrate");
    let mailer: Arc<dyn Mailer> = match &config.resend_api_key {
        Some(key) => Arc::new(ResendMailer { api_key: key.clone(), http: reqwest::Client::new() }),
        None => Arc::new(LogMailer),
    };
    let state = AppState {
        pool,
        config: Arc::new(config.clone()),
        mailer,
        limiter: Arc::new(RateLimiter::new(3, Duration::from_secs(900))),
    };
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await.unwrap();
    tracing::info!("listening on {}", config.bind_addr);
    axum::serve(listener, peetopee_api::app(state)).await.unwrap();
}
```

- [ ] **Step 3: Write test helper + failing tests**

`api/tests/common/mod.rs`:
```rust
use std::sync::Arc;
use std::time::Duration;
use peetopee_api::auth::limiter::RateLimiter;
use peetopee_api::auth::mailer::MemoryMailer;
use peetopee_api::{config::Config, state::AppState};
use sqlx::PgPool;

pub fn test_state(pool: PgPool) -> (AppState, Arc<MemoryMailer>) {
    let mailer = Arc::new(MemoryMailer::default());
    let config = Config {
        database_url: String::new(),
        base_url: "http://localhost:3000".into(),
        bind_addr: String::new(),
        resend_api_key: None,
        etherscan_api_key: None,
        trongrid_api_key: None,
    };
    let state = AppState {
        pool,
        config: Arc::new(config),
        mailer: mailer.clone(),
        limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(900))),
    };
    (state, mailer)
}

/// Pull the token query param out of a captured magic link.
pub fn extract_token(link: &str) -> String {
    link.split("token=").nth(1).expect("link has token").to_string()
}
```

`api/tests/auth_test.rs`:
```rust
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
```

- [ ] **Step 4: Run tests**

Run: `cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add api
git commit -m "feat: magic-link request with resend mailer and rate limiting"
```

---

### Task 4: Verify, sessions, logout, CurrentUser extractor

**Files:**
- Modify: `api/src/auth/mod.rs`, `api/src/auth/session.rs`, `api/src/lib.rs`
- Test: `api/tests/auth_test.rs` (append), `api/tests/common/mod.rs` (append)

**Interfaces:**
- Consumes: Task 3's `new_token`/`hash_token`, `AppState`.
- Produces: routes `GET /api/auth/verify?token=`, `POST /api/auth/logout`; `auth::session::CurrentUser(pub User)` axum extractor; `auth::session::User { id: Uuid, email: String, telegram_handle: Option<String>, usdt_trc20: Option<String>, usdt_bep20: Option<String>, usdt_erc20: Option<String> }` (FromRow + Serialize); test helper `common::login(app: &Router, mailer: &MemoryMailer, email: &str) -> String` returning a `Cookie` header value.

- [ ] **Step 1: Add User + CurrentUser to session.rs**

Append to `api/src/auth/session.rs`:
```rust
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;
use serde::Serialize;
use uuid::Uuid;
use crate::error::ApiError;
use crate::state::AppState;

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
```

- [ ] **Step 2: Add verify + logout handlers**

Append to `api/src/auth/mod.rs`:
```rust
use axum::extract::Query;
use axum::response::Redirect;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use uuid::Uuid;
use session::CurrentUser;

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
```

Modify `api/src/lib.rs` router (add routes):
```rust
        .route("/api/auth/verify", get(auth::verify))
        .route("/api/auth/logout", post(auth::logout))
```

- [ ] **Step 3: Add login helper + tests**

Append to `api/tests/common/mod.rs`:
```rust
use axum::body::Body;
use axum::http::Request;
use axum::Router;
use tower::ServiceExt;

/// Full magic-link login; returns the `Cookie` header value ("session=...").
pub async fn login(app: &Router, mailer: &MemoryMailer, email: &str) -> String {
    let res = app.clone()
        .oneshot(
            Request::post("/api/auth/request-link")
                .header("content-type", "application/json")
                .body(Body::from(format!("{{\"email\":\"{email}\"}}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(res.status().is_success());
    let link = mailer.sent.lock().unwrap().last().unwrap().1.clone();
    let token = extract_token(&link);
    let res = app.clone()
        .oneshot(Request::get(format!("/api/auth/verify?token={token}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let set_cookie = res.headers().get("set-cookie").expect("set-cookie").to_str().unwrap();
    set_cookie.split(';').next().unwrap().to_string()
}
```

Append to `api/tests/auth_test.rs`:
```rust
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
```

- [ ] **Step 4: Run tests**

Run: `cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
Expected: all pass. Note axum `Redirect::to` responds 303 SEE_OTHER.

- [ ] **Step 5: Commit**

```bash
git add api
git commit -m "feat: magic-link verify, sessions, logout, CurrentUser extractor"
```

---

### Task 5: Profile endpoints (GET/PATCH /api/me)

**Files:**
- Create: `api/src/users.rs`
- Modify: `api/src/lib.rs`
- Test: `api/tests/users_test.rs`

**Interfaces:**
- Consumes: `CurrentUser`, `User`, `AppState`, `ApiError`.
- Produces: routes `GET /api/me` → `User` JSON; `PATCH /api/me` body `{telegram_handle: string (required), usdt_trc20?: string|null, usdt_bep20?: string|null, usdt_erc20?: string|null}` → updated `User`. Validation: telegram `^[A-Za-z0-9_]{5,32}$` after stripping a leading `@`; TRC20 `^T[1-9A-HJ-NP-Za-km-z]{33}$`; BEP20/ERC20 `^0x[0-9a-fA-F]{40}$`. Empty-string address = clear (NULL).

- [ ] **Step 1: Write users.rs**

`api/src/users.rs`:
```rust
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

#[derive(Deserialize)]
pub struct UpdateMe {
    pub telegram_handle: Option<String>,
    pub usdt_trc20: Option<String>,
    pub usdt_bep20: Option<String>,
    pub usdt_erc20: Option<String>,
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
        .map(|s| s.trim().trim_start_matches('@').to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::BadRequest("telegram_handle is required".into()))?;
    if !TG_RE.is_match(&tg) {
        return Err(ApiError::BadRequest("invalid telegram handle (5-32 letters, digits, underscore)".into()));
    }
    let trc20 = clean_addr(body.usdt_trc20, &TRC20_RE, "TRC20")?;
    let bep20 = clean_addr(body.usdt_bep20, &EVM_RE, "BEP20")?;
    let erc20 = clean_addr(body.usdt_erc20, &EVM_RE, "ERC20")?;
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
```

Modify `api/src/lib.rs`: add `pub mod users;` and routes:
```rust
        .route("/api/me", get(users::get_me).patch(users::update_me))
```

- [ ] **Step 2: Write tests**

`api/tests/users_test.rs`:
```rust
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
    assert_eq!(v["usdt_bep20"], serde_json::Value::Null);
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
```

- [ ] **Step 3: Run tests**

Run: `cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add api
git commit -m "feat: profile endpoints with telegram and usdt address validation"
```

---

### Task 6: Orders — create, list mine, list open, detail

**Files:**
- Create: `api/src/orders/mod.rs`, `api/src/orders/model.rs`
- Modify: `api/src/lib.rs`
- Test: `api/tests/orders_test.rs`

**Interfaces:**
- Consumes: `CurrentUser`, `User`, `AppState`, `ApiError`.
- Produces: `orders::model::{CURRENCIES: [&str; 8], Order, OrderDetail, CourierUsdt, ORDER_COLUMNS: &str}`; routes `POST /api/orders` (body `{fiat_currency, fiat_amount: string, usdt_amount: string, address_text, lat, lng}`), `GET /api/orders/mine`, `GET /api/orders/open`, `GET /api/orders/{id}`. Task 7 adds transition handlers to `orders::mod`.

- [ ] **Step 1: Write model.rs**

`api/src/orders/model.rs`:
```rust
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
```

- [ ] **Step 2: Write handlers**

`api/src/orders/mod.rs`:
```rust
pub mod model;

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
    if addr.is_empty() || addr.len() > 500 {
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
```

Modify `api/src/lib.rs`: add `pub mod orders;` and routes:
```rust
        .route("/api/orders", post(orders::create))
        .route("/api/orders/mine", get(orders::list_mine))
        .route("/api/orders/open", get(orders::list_open))
        .route("/api/orders/{id}", get(orders::get_detail))
```

- [ ] **Step 3: Write tests**

`api/tests/orders_test.rs`:
```rust
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
    assert_eq!(order["usdt_amount"], "92.5");

    let res = app.clone().oneshot(get_req("/api/orders/mine", &cookie)).await.unwrap();
    assert_eq!(body_json(res).await.as_array().unwrap().len(), 1);

    // customer's own order not on their courier board
    let res = app.clone().oneshot(get_req("/api/orders/open", &cookie)).await.unwrap();
    assert_eq!(body_json(res).await.as_array().unwrap().len(), 0);

    // but visible on another user's board
    let cookie2 = common::login(&app, &mailer, "cour@example.com").await;
    let res = app.clone().oneshot(get_req("/api/orders/open", &cookie2)).await.unwrap();
    assert_eq!(body_json(res).await.as_array().unwrap().len(), 1);
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
}
```

- [ ] **Step 4: Run tests**

Run: `cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add api
git commit -m "feat: order creation, listings, and detail with field redaction"
```

---

### Task 7: Order transitions (accept / request-payment / confirm-cash / cancel)

**Files:**
- Create: `api/src/orders/transitions.rs`
- Modify: `api/src/orders/mod.rs` (add `pub mod transitions;`), `api/src/lib.rs`
- Test: `api/tests/transitions_test.rs`

**Interfaces:**
- Consumes: Task 6's `Order`, `ORDER_COLUMNS`, `CurrentUser`, `AppState`.
- Produces: routes `POST /api/orders/{id}/accept`, `POST /api/orders/{id}/request-payment`, `POST /api/orders/{id}/confirm-cash`, `POST /api/orders/{id}/cancel`, each returning updated `Order` JSON. All guards live in the SQL `WHERE` (compare-and-swap on `status`) so concurrent requests can't double-transition.

- [ ] **Step 1: Write transitions.rs**

`api/src/orders/transitions.rs`:
```rust
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
```

Add `pub mod transitions;` at the top of `api/src/orders/mod.rs`.

Modify `api/src/lib.rs`: add routes:
```rust
        .route("/api/orders/{id}/accept", post(orders::transitions::accept))
        .route("/api/orders/{id}/request-payment", post(orders::transitions::request_payment))
        .route("/api/orders/{id}/confirm-cash", post(orders::transitions::confirm_cash))
        .route("/api/orders/{id}/cancel", post(orders::transitions::cancel))
```

- [ ] **Step 2: Write tests**

`api/tests/transitions_test.rs`:
```rust
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

/// Creates customer + courier (courier has a TRC20 address) and one OPEN order.
/// Returns (customer_cookie, courier_cookie, order_id).
async fn setup(app: &axum::Router, mailer: &peetopee_api::auth::mailer::MemoryMailer) -> (String, String, String) {
    let cust = common::login(app, mailer, "cust@example.com").await;
    let cour = common::login(app, mailer, "cour@example.com").await;
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
```

- [ ] **Step 3: Run tests**

Run: `cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add api
git commit -m "feat: order state transitions with SQL compare-and-swap guards"
```

---

### Task 8: Chain watcher (TRC20 / BEP20 / ERC20)

**Files:**
- Create: `api/src/watcher/mod.rs`, `api/src/watcher/matching.rs`, `api/src/watcher/clients.rs`
- Modify: `api/src/lib.rs` (add `pub mod watcher;`), `api/src/main.rs` (spawn watcher)
- Test: unit tests inline in `matching.rs` and `clients.rs`

**Interfaces:**
- Consumes: `AppState`, orders in `AWAITING_PAYMENT`, couriers' `usdt_*` columns.
- Produces: `watcher::run(state: AppState)` (never returns; spawned from main); `matching::{IncomingTransfer{txid,to,amount:Decimal,timestamp:DateTime<Utc>}, find_matching_transfer(...)}`; `clients::{ChainClient::new(config:&Config), trc20_transfers(addr,since), evm_transfers(chain_id,usdt_contract,addr), parse_trongrid(&Value), parse_etherscan(&Value), USDT_TRC20, USDT_ERC20, USDT_BEP20}`.

- [ ] **Step 1: Write matching.rs (pure logic + tests)**

`api/src/watcher/matching.rs`:
```rust
use std::collections::HashSet;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct IncomingTransfer {
    pub txid: String,
    pub to: String,
    pub amount: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// First transfer to `courier_address` of at least `min_amount`, at/after
/// `requested_at`, whose txid hasn't already settled another order.
pub fn find_matching_transfer<'a>(
    transfers: &'a [IncomingTransfer],
    courier_address: &str,
    min_amount: Decimal,
    requested_at: DateTime<Utc>,
    used_txids: &HashSet<String>,
) -> Option<&'a IncomingTransfer> {
    transfers.iter().find(|t| {
        t.to.eq_ignore_ascii_case(courier_address)
            && t.amount >= min_amount
            && t.timestamp >= requested_at
            && !used_txids.contains(&t.txid)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn t(txid: &str, to: &str, amount: &str, secs: i64) -> IncomingTransfer {
        IncomingTransfer {
            txid: txid.into(),
            to: to.into(),
            amount: Decimal::from_str(amount).unwrap(),
            timestamp: DateTime::from_timestamp(secs, 0).unwrap(),
        }
    }

    const ADDR: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";

    #[test]
    fn matches_exact_amount_after_request() {
        let transfers = [t("tx1", ADDR, "92.5", 1000)];
        let hit = find_matching_transfer(&transfers, ADDR, Decimal::from_str("92.5").unwrap(),
            DateTime::from_timestamp(900, 0).unwrap(), &HashSet::new());
        assert_eq!(hit.unwrap().txid, "tx1");
    }

    #[test]
    fn rejects_low_amount_early_timestamp_wrong_address_and_used_txid() {
        let requested = DateTime::from_timestamp(900, 0).unwrap();
        let min = Decimal::from_str("92.5").unwrap();
        assert!(find_matching_transfer(&[t("a", ADDR, "92.4", 1000)], ADDR, min, requested, &HashSet::new()).is_none());
        assert!(find_matching_transfer(&[t("b", ADDR, "92.5", 800)], ADDR, min, requested, &HashSet::new()).is_none());
        assert!(find_matching_transfer(&[t("c", "TOtherAddr", "92.5", 1000)], ADDR, min, requested, &HashSet::new()).is_none());
        let used: HashSet<String> = ["d".to_string()].into();
        assert!(find_matching_transfer(&[t("d", ADDR, "92.5", 1000)], ADDR, min, requested, &used).is_none());
    }

    #[test]
    fn evm_address_match_is_case_insensitive() {
        let transfers = [t("e", "0xdac17f958d2ee523a2206206994597c13d831ec7", "50", 1000)];
        let hit = find_matching_transfer(&transfers, "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            Decimal::from_str("50").unwrap(), DateTime::from_timestamp(900, 0).unwrap(), &HashSet::new());
        assert!(hit.is_some());
    }
}
```

- [ ] **Step 2: Write clients.rs (HTTP clients + parsers + tests)**

`api/src/watcher/clients.rs`:
```rust
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use crate::config::Config;
use super::matching::IncomingTransfer;

pub const USDT_TRC20: &str = "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t";
pub const USDT_ERC20: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7";
pub const USDT_BEP20: &str = "0x55d398326f99059fF775485246999027B3197955";

pub struct ChainClient {
    http: reqwest::Client,
    etherscan_key: Option<String>,
    trongrid_key: Option<String>,
}

impl ChainClient {
    pub fn new(config: &Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            etherscan_key: config.etherscan_api_key.clone(),
            trongrid_key: config.trongrid_api_key.clone(),
        }
    }

    pub async fn trc20_transfers(&self, address: &str, since: DateTime<Utc>) -> Result<Vec<IncomingTransfer>, String> {
        let url = format!(
            "https://api.trongrid.io/v1/accounts/{address}/transactions/trc20\
             ?only_to=true&contract_address={USDT_TRC20}&min_timestamp={}&limit=50",
            since.timestamp_millis()
        );
        let mut req = self.http.get(&url);
        if let Some(k) = &self.trongrid_key {
            req = req.header("TRON-PRO-API-KEY", k);
        }
        let v: Value = req.send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
        Ok(parse_trongrid(&v))
    }

    pub async fn evm_transfers(&self, chain_id: u32, usdt_contract: &str, address: &str) -> Result<Vec<IncomingTransfer>, String> {
        let key = self.etherscan_key.clone().unwrap_or_default();
        let url = format!(
            "https://api.etherscan.io/v2/api?chainid={chain_id}&module=account&action=tokentx\
             &contractaddress={usdt_contract}&address={address}&page=1&offset=50&sort=desc&apikey={key}"
        );
        let v: Value = self.http.get(&url).send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
        Ok(parse_etherscan(&v))
    }
}

/// TronGrid TRC20 payload: data[].{transaction_id, to, value (int string, 6 dp), block_timestamp (ms)}
pub fn parse_trongrid(v: &Value) -> Vec<IncomingTransfer> {
    v["data"].as_array().map(|arr| {
        arr.iter().filter_map(|t| {
            Some(IncomingTransfer {
                txid: t["transaction_id"].as_str()?.to_string(),
                to: t["to"].as_str()?.to_string(),
                amount: Decimal::from_i128_with_scale(t["value"].as_str()?.parse::<i128>().ok()?, 6),
                timestamp: DateTime::from_timestamp_millis(t["block_timestamp"].as_i64()?)?,
            })
        }).collect()
    }).unwrap_or_default()
}

/// Etherscan V2 tokentx payload: result[].{hash, to, value (int string), tokenDecimal, timeStamp (s)}
pub fn parse_etherscan(v: &Value) -> Vec<IncomingTransfer> {
    v["result"].as_array().map(|arr| {
        arr.iter().filter_map(|t| {
            let decimals: u32 = t["tokenDecimal"].as_str()?.parse().ok()?;
            Some(IncomingTransfer {
                txid: t["hash"].as_str()?.to_string(),
                to: t["to"].as_str()?.to_string(),
                amount: Decimal::from_i128_with_scale(t["value"].as_str()?.parse::<i128>().ok()?, decimals),
                timestamp: DateTime::from_timestamp(t["timeStamp"].as_str()?.parse::<i64>().ok()?, 0)?,
            })
        }).collect()
    }).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_trongrid_fixture() {
        let v: Value = serde_json::from_str(r#"{
            "data": [{
                "transaction_id": "aabbcc",
                "to": "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t",
                "from": "TSender",
                "value": "92500000",
                "block_timestamp": 1752700000000,
                "token_info": {"symbol": "USDT"}
            }]
        }"#).unwrap();
        let transfers = parse_trongrid(&v);
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].amount, Decimal::from_str("92.5").unwrap());
        assert_eq!(transfers[0].txid, "aabbcc");
    }

    #[test]
    fn parses_etherscan_fixture_with_18_decimals() {
        let v: Value = serde_json::from_str(r#"{
            "status": "1",
            "result": [{
                "hash": "0xdeadbeef",
                "to": "0x55d398326f99059ff775485246999027b3197955",
                "value": "92500000000000000000",
                "tokenDecimal": "18",
                "timeStamp": "1752700000"
            }]
        }"#).unwrap();
        let transfers = parse_etherscan(&v);
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].amount, Decimal::from_str("92.5").unwrap());
    }

    #[test]
    fn tolerates_error_payloads() {
        let v: Value = serde_json::from_str(r#"{"status":"0","message":"NOTOK","result":"Max rate limit reached"}"#).unwrap();
        assert!(parse_etherscan(&v).is_empty());
        let v: Value = serde_json::from_str(r#"{"success": false}"#).unwrap();
        assert!(parse_trongrid(&v).is_empty());
    }
}
```

- [ ] **Step 3: Write watcher loop**

`api/src/watcher/mod.rs`:
```rust
pub mod clients;
pub mod matching;

use std::collections::HashSet;
use std::time::Duration;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use uuid::Uuid;
use crate::state::AppState;
use clients::{ChainClient, USDT_BEP20, USDT_ERC20};
use matching::find_matching_transfer;

pub async fn run(state: AppState) {
    let client = ChainClient::new(&state.config);
    let mut tick = tokio::time::interval(Duration::from_secs(20));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        if let Err(e) = poll_once(&state, &client).await {
            tracing::warn!("watcher poll failed: {e}");
        }
    }
}

struct PendingOrder {
    id: Uuid,
    usdt_amount: Decimal,
    requested_at: DateTime<Utc>,
    trc20: Option<String>,
    bep20: Option<String>,
    erc20: Option<String>,
}

async fn poll_once(state: &AppState, client: &ChainClient) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT o.id, o.usdt_amount, o.payment_requested_at, u.usdt_trc20, u.usdt_bep20, u.usdt_erc20 \
         FROM orders o JOIN users u ON u.id = o.courier_id \
         WHERE o.status = 'AWAITING_PAYMENT'",
    )
    .fetch_all(&state.pool).await.map_err(|e| e.to_string())?;
    if rows.is_empty() {
        return Ok(());
    }
    let used_txids: HashSet<String> = sqlx::query_scalar::<_, String>(
        "SELECT payment_txid FROM orders WHERE payment_txid IS NOT NULL",
    )
    .fetch_all(&state.pool).await.map_err(|e| e.to_string())?
    .into_iter().collect();

    for row in rows {
        let order = PendingOrder {
            id: row.get("id"),
            usdt_amount: row.get("usdt_amount"),
            requested_at: row.get("payment_requested_at"),
            trc20: row.get("usdt_trc20"),
            bep20: row.get("usdt_bep20"),
            erc20: row.get("usdt_erc20"),
        };
        check_order(state, client, &order, &used_txids).await;
    }
    Ok(())
}

async fn check_order(state: &AppState, client: &ChainClient, order: &PendingOrder, used: &HashSet<String>) {
    let mut attempts: Vec<(&str, Result<Vec<matching::IncomingTransfer>, String>, &str)> = Vec::new();
    if let Some(addr) = &order.trc20 {
        attempts.push(("trc20", client.trc20_transfers(addr, order.requested_at).await, addr.as_str()));
    }
    if let Some(addr) = &order.bep20 {
        attempts.push(("bep20", client.evm_transfers(56, USDT_BEP20, addr).await, addr.as_str()));
    }
    if let Some(addr) = &order.erc20 {
        attempts.push(("erc20", client.evm_transfers(1, USDT_ERC20, addr).await, addr.as_str()));
    }
    for (network, result, addr) in attempts {
        match result {
            Err(e) => tracing::warn!("watcher {network} fetch failed for order {}: {e}", order.id),
            Ok(transfers) => {
                if let Some(hit) = find_matching_transfer(&transfers, addr, order.usdt_amount, order.requested_at, used) {
                    let updated = sqlx::query(
                        "UPDATE orders SET status = 'PAID', payment_network = $1, payment_txid = $2, paid_at = now() \
                         WHERE id = $3 AND status = 'AWAITING_PAYMENT'",
                    )
                    .bind(network).bind(&hit.txid).bind(order.id)
                    .execute(&state.pool).await;
                    match updated {
                        Ok(r) if r.rows_affected() == 1 => {
                            tracing::info!("order {} paid via {network} tx {}", order.id, hit.txid);
                            return;
                        }
                        Ok(_) => {}
                        Err(e) => tracing::error!("watcher settle failed for {}: {e}", order.id),
                    }
                }
            }
        }
    }
}
```

Modify `api/src/lib.rs`: add `pub mod watcher;`.
Modify `api/src/main.rs`: after building `state`, before `axum::serve`:
```rust
    tokio::spawn(peetopee_api::watcher::run(state.clone()));
```

- [ ] **Step 4: Run tests**

Run: `cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test`
Expected: all pass, including new `matching::tests` and `clients::tests` units.

- [ ] **Step 5: Commit**

```bash
git add api
git commit -m "feat: usdt chain watcher for trc20/bep20/erc20 with pure matching logic"
```

---

### Task 9: Next.js scaffold, API client, auth pages

**Files:**
- Create: `web/package.json`, `web/tsconfig.json`, `web/next.config.ts`, `web/postcss.config.mjs`, `web/next-env.d.ts` (generated), `web/src/app/globals.css`, `web/src/app/layout.tsx`, `web/src/lib/api.ts`, `web/src/lib/types.ts`, `web/src/lib/useUser.ts`, `web/src/components/Nav.tsx`, `web/src/components/ProfileForm.tsx`, `web/src/app/login/page.tsx`, `web/src/app/onboarding/page.tsx`, `web/src/app/settings/page.tsx`, `web/public/.gitkeep`

**Interfaces:**
- Consumes: API routes from Tasks 3-5.
- Produces: `lib/api.ts` `api<T>(path, init?)` throwing `ApiError{status,message}`; `lib/types.ts` (`Currency`, `CURRENCIES`, `OrderStatus`, `User`, `Order`, `OrderDetail`, `Network`); `useUser()` hook (fetches `/me`, redirects to `/login` on 401); `<ProfileForm initial onSaved>` shared by onboarding + settings; `<Nav/>` in layout. Tasks 10-12 import all of these.

- [ ] **Step 1: Write config files**

`web/package.json`:
```json
{
  "name": "peetopee-web",
  "private": true,
  "scripts": {
    "dev": "next dev",
    "build": "next build",
    "start": "next start",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "leaflet": "^1.9.4",
    "next": "^15.3.4",
    "qrcode.react": "^4.2.0",
    "react": "^19.1.0",
    "react-dom": "^19.1.0",
    "react-leaflet": "^5.0.0"
  },
  "devDependencies": {
    "@tailwindcss/postcss": "^4.1.10",
    "@types/leaflet": "^1.9.18",
    "@types/node": "^22",
    "@types/react": "^19",
    "@types/react-dom": "^19",
    "tailwindcss": "^4.1.10",
    "typescript": "^5"
  }
}
```

`web/tsconfig.json`:
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "lib": ["dom", "dom.iterable", "esnext"],
    "allowJs": false,
    "skipLibCheck": true,
    "strict": true,
    "noEmit": true,
    "esModuleInterop": true,
    "module": "esnext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "preserve",
    "incremental": true,
    "plugins": [{ "name": "next" }],
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["next-env.d.ts", "**/*.ts", "**/*.tsx", ".next/types/**/*.ts"],
  "exclude": ["node_modules"]
}
```

`web/next.config.ts`:
```ts
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  async rewrites() {
    if (process.env.NODE_ENV === "development") {
      return [{ source: "/api/:path*", destination: "http://localhost:8080/api/:path*" }];
    }
    return [];
  },
};

export default nextConfig;
```

`web/postcss.config.mjs`:
```js
export default { plugins: { "@tailwindcss/postcss": {} } };
```

`web/src/app/globals.css`:
```css
@import "tailwindcss";
```

- [ ] **Step 2: Write lib (api, types, useUser)**

`web/src/lib/types.ts`:
```ts
export type Currency = "IDR" | "AED" | "EUR" | "GBP" | "RUB" | "INR" | "USD" | "CAD";
export const CURRENCIES: Currency[] = ["IDR", "AED", "EUR", "GBP", "RUB", "INR", "USD", "CAD"];

export type OrderStatus = "OPEN" | "ACCEPTED" | "AWAITING_PAYMENT" | "PAID" | "COMPLETED" | "CANCELLED";
export type Network = "trc20" | "bep20" | "erc20";

export interface User {
  id: string;
  email: string;
  telegram_handle: string | null;
  usdt_trc20: string | null;
  usdt_bep20: string | null;
  usdt_erc20: string | null;
}

export interface Order {
  id: string;
  customer_id: string;
  courier_id: string | null;
  fiat_currency: Currency;
  fiat_amount: string;
  usdt_amount: string;
  address_text: string;
  lat: number;
  lng: number;
  status: OrderStatus;
  payment_network: Network | null;
  payment_txid: string | null;
  payment_requested_at: string | null;
  paid_at: string | null;
  created_at: string;
  accepted_at: string | null;
  completed_at: string | null;
  cancelled_at: string | null;
}

export interface OrderDetail extends Order {
  is_customer: boolean;
  is_courier: boolean;
  customer_telegram: string | null;
  courier_telegram: string | null;
  courier_usdt: { trc20: string | null; bep20: string | null; erc20: string | null } | null;
}
```

`web/src/lib/api.ts`:
```ts
export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    ...init,
    headers: { "Content-Type": "application/json", ...(init?.headers ?? {}) },
    credentials: "same-origin",
  });
  if (!res.ok) {
    let msg = res.statusText;
    try {
      msg = ((await res.json()) as { error?: string }).error ?? msg;
    } catch {}
    throw new ApiError(res.status, msg);
  }
  return (await res.json()) as T;
}
```

`web/src/lib/useUser.ts`:
```ts
"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "./api";
import type { User } from "./types";

export function useUser(options: { redirect?: boolean } = {}) {
  const { redirect = true } = options;
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const router = useRouter();

  useEffect(() => {
    api<User>("/me")
      .then(setUser)
      .catch((e: unknown) => {
        if (redirect && e instanceof ApiError && e.status === 401) router.push("/login");
      })
      .finally(() => setLoading(false));
  }, [redirect, router]);

  return { user, loading };
}
```

- [ ] **Step 3: Write layout, Nav, ProfileForm, auth pages**

`web/src/app/layout.tsx`:
```tsx
import type { Metadata } from "next";
import "./globals.css";
import Nav from "@/components/Nav";

export const metadata: Metadata = {
  title: "peetopee.cash",
  description: "Cash delivered to your door, paid in USDT.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-zinc-950 text-zinc-100 antialiased">
        <Nav />
        <main className="mx-auto max-w-3xl px-4 py-8">{children}</main>
      </body>
    </html>
  );
}
```

`web/src/components/Nav.tsx`:
```tsx
"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { useUser } from "@/lib/useUser";

export default function Nav() {
  const { user } = useUser({ redirect: false });
  const router = useRouter();

  async function logout() {
    await api("/auth/logout", { method: "POST" }).catch(() => {});
    router.push("/login");
    router.refresh();
  }

  return (
    <nav className="border-b border-zinc-800 bg-zinc-900/60">
      <div className="mx-auto flex max-w-3xl items-center gap-5 px-4 py-3 text-sm">
        <Link href="/" className="font-bold text-emerald-400">peetopee.cash</Link>
        {user ? (
          <>
            <Link href="/orders" className="hover:text-emerald-300">My orders</Link>
            <Link href="/orders/new" className="hover:text-emerald-300">Get cash</Link>
            <Link href="/courier" className="hover:text-emerald-300">Courier board</Link>
            <span className="ml-auto flex items-center gap-4">
              <Link href="/settings" className="text-zinc-400 hover:text-emerald-300">Settings</Link>
              <button onClick={logout} className="text-zinc-400 hover:text-red-400">Log out</button>
            </span>
          </>
        ) : (
          <Link href="/login" className="ml-auto hover:text-emerald-300">Log in</Link>
        )}
      </div>
    </nav>
  );
}
```

`web/src/components/ProfileForm.tsx`:
```tsx
"use client";

import { useState } from "react";
import { api } from "@/lib/api";
import type { User } from "@/lib/types";

const FIELDS = [
  { key: "usdt_trc20", label: "USDT address — TRC20 (Tron)", placeholder: "T..." },
  { key: "usdt_bep20", label: "USDT address — BEP20 (BNB Smart Chain)", placeholder: "0x..." },
  { key: "usdt_erc20", label: "USDT address — ERC20 (Ethereum)", placeholder: "0x..." },
] as const;

export default function ProfileForm({ initial, onSaved }: { initial: User; onSaved: (u: User) => void }) {
  const [telegram, setTelegram] = useState(initial.telegram_handle ?? "");
  const [addrs, setAddrs] = useState({
    usdt_trc20: initial.usdt_trc20 ?? "",
    usdt_bep20: initial.usdt_bep20 ?? "",
    usdt_erc20: initial.usdt_erc20 ?? "",
  });
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const updated = await api<User>("/me", {
        method: "PATCH",
        body: JSON.stringify({ telegram_handle: telegram, ...addrs }),
      });
      onSaved(updated);
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to save");
    } finally {
      setSaving(false);
    }
  }

  return (
    <form onSubmit={save} className="space-y-4">
      <label className="block">
        <span className="mb-1 block text-sm text-zinc-400">Telegram handle (required — the other party contacts you here)</span>
        <input
          value={telegram}
          onChange={(e) => setTelegram(e.target.value)}
          placeholder="@yourhandle"
          required
          className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
        />
      </label>
      {FIELDS.map((f) => (
        <label key={f.key} className="block">
          <span className="mb-1 block text-sm text-zinc-400">{f.label}</span>
          <input
            value={addrs[f.key]}
            onChange={(e) => setAddrs({ ...addrs, [f.key]: e.target.value })}
            placeholder={f.placeholder}
            className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2 font-mono text-sm"
          />
        </label>
      ))}
      <p className="text-xs text-zinc-500">At least one USDT address is required to accept orders as a courier.</p>
      {error && <p className="text-sm text-red-400">{error}</p>}
      <button disabled={saving} className="rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
        {saving ? "Saving..." : "Save"}
      </button>
    </form>
  );
}
```

`web/src/app/login/page.tsx`:
```tsx
"use client";

import { Suspense, useState } from "react";
import { useSearchParams } from "next/navigation";
import { api } from "@/lib/api";

function LoginForm() {
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const params = useSearchParams();

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      await api("/auth/request-link", { method: "POST", body: JSON.stringify({ email }) });
      setSent(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed");
    }
  }

  if (sent) {
    return (
      <div className="mx-auto max-w-sm pt-16 text-center">
        <h1 className="text-2xl font-bold">Check your inbox</h1>
        <p className="mt-3 text-zinc-400">We sent a login link to <span className="text-zinc-200">{email}</span>. It expires in 15 minutes.</p>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-sm pt-16">
      <h1 className="text-2xl font-bold">Log in or sign up</h1>
      <p className="mt-2 text-sm text-zinc-400">Enter your email and we&apos;ll send you a magic link. No password needed.</p>
      {params.get("error") === "expired" && (
        <p className="mt-3 rounded bg-amber-950 px-3 py-2 text-sm text-amber-300">That link was expired or already used — request a new one.</p>
      )}
      <form onSubmit={submit} className="mt-6 space-y-3">
        <input
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder="you@example.com"
          required
          className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
        />
        {error && <p className="text-sm text-red-400">{error}</p>}
        <button className="w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500">Send magic link</button>
      </form>
    </div>
  );
}

export default function LoginPage() {
  return (
    <Suspense>
      <LoginForm />
    </Suspense>
  );
}
```

`web/src/app/onboarding/page.tsx`:
```tsx
"use client";

import { useRouter } from "next/navigation";
import ProfileForm from "@/components/ProfileForm";
import { useUser } from "@/lib/useUser";

export default function OnboardingPage() {
  const { user, loading } = useUser();
  const router = useRouter();

  if (loading || !user) return <p className="text-zinc-500">Loading...</p>;

  return (
    <div className="mx-auto max-w-md">
      <h1 className="text-2xl font-bold">Welcome 👋</h1>
      <p className="mb-6 mt-2 text-sm text-zinc-400">
        Set your Telegram handle so customers and couriers can coordinate meetups with you.
      </p>
      <ProfileForm initial={user} onSaved={() => router.push("/orders")} />
    </div>
  );
}
```

`web/src/app/settings/page.tsx`:
```tsx
"use client";

import { useState } from "react";
import ProfileForm from "@/components/ProfileForm";
import { useUser } from "@/lib/useUser";

export default function SettingsPage() {
  const { user, loading } = useUser();
  const [saved, setSaved] = useState(false);

  if (loading || !user) return <p className="text-zinc-500">Loading...</p>;

  return (
    <div className="mx-auto max-w-md">
      <h1 className="mb-6 text-2xl font-bold">Settings</h1>
      <ProfileForm initial={user} onSaved={() => setSaved(true)} />
      {saved && <p className="mt-3 text-sm text-emerald-400">Saved.</p>}
    </div>
  );
}
```

- [ ] **Step 4: Install and build**

Run: `cd web && npm install && npm run build`
Expected: build succeeds (this also generates `next-env.d.ts`). Fix any type errors before proceeding.

- [ ] **Step 5: Commit**

```bash
git add web
git commit -m "feat: nextjs scaffold with magic-link auth pages and profile form"
```

---

### Task 10: Order form with map picker + my-orders list

**Files:**
- Create: `web/src/components/MapPicker.tsx`, `web/src/components/OrderCard.tsx`, `web/src/components/StatusBadge.tsx`, `web/src/app/orders/new/page.tsx`, `web/src/app/orders/page.tsx`

**Interfaces:**
- Consumes: `api`, `types`, `useUser` from Task 9; `POST /api/orders`, `GET /api/orders/mine`.
- Produces: `<MapPicker value={{lat,lng}} onChange>` (client-only Leaflet map, draggable pin + click-to-place); `<OrderCard order/>` linking to `/orders/{id}`; `<StatusBadge status/>`. Task 11 reuses OrderCard and StatusBadge.

- [ ] **Step 1: Write MapPicker**

`web/src/components/MapPicker.tsx`:
```tsx
"use client";

import { MapContainer, TileLayer, Marker, useMapEvents } from "react-leaflet";
import "leaflet/dist/leaflet.css";
import L from "leaflet";

// divIcon avoids Leaflet's default marker asset paths, which break under bundlers
const pin = L.divIcon({ className: "", html: "<div style='font-size:28px;line-height:1'>📍</div>", iconSize: [28, 28], iconAnchor: [14, 28] });

type Point = { lat: number; lng: number };

function ClickHandler({ onChange }: { onChange: (p: Point) => void }) {
  useMapEvents({
    click(e) {
      onChange({ lat: e.latlng.lat, lng: e.latlng.lng });
    },
  });
  return null;
}

export default function MapPicker({ value, onChange }: { value: Point; onChange: (p: Point) => void }) {
  return (
    <MapContainer center={[value.lat, value.lng]} zoom={13} className="h-64 w-full rounded border border-zinc-700">
      <TileLayer url="https://tile.openstreetmap.org/{z}/{x}/{y}.png" attribution="&copy; OpenStreetMap contributors" />
      <Marker
        position={[value.lat, value.lng]}
        draggable
        icon={pin}
        eventHandlers={{
          dragend: (e) => {
            const p = (e.target as L.Marker).getLatLng();
            onChange({ lat: p.lat, lng: p.lng });
          },
        }}
      />
      <ClickHandler onChange={onChange} />
    </MapContainer>
  );
}
```

- [ ] **Step 2: Write StatusBadge + OrderCard**

`web/src/components/StatusBadge.tsx`:
```tsx
import type { OrderStatus } from "@/lib/types";

const STYLES: Record<OrderStatus, string> = {
  OPEN: "bg-sky-950 text-sky-300",
  ACCEPTED: "bg-indigo-950 text-indigo-300",
  AWAITING_PAYMENT: "bg-amber-950 text-amber-300",
  PAID: "bg-emerald-950 text-emerald-300",
  COMPLETED: "bg-emerald-900 text-emerald-200",
  CANCELLED: "bg-zinc-800 text-zinc-400",
};

const LABELS: Record<OrderStatus, string> = {
  OPEN: "Open",
  ACCEPTED: "Accepted",
  AWAITING_PAYMENT: "Awaiting payment",
  PAID: "Paid",
  COMPLETED: "Completed",
  CANCELLED: "Cancelled",
};

export default function StatusBadge({ status }: { status: OrderStatus }) {
  return <span className={`rounded px-2 py-0.5 text-xs font-medium ${STYLES[status]}`}>{LABELS[status]}</span>;
}
```

`web/src/components/OrderCard.tsx`:
```tsx
import Link from "next/link";
import type { Order } from "@/lib/types";
import StatusBadge from "./StatusBadge";

export function impliedRate(order: Order): string {
  const fiat = parseFloat(order.fiat_amount);
  const usdt = parseFloat(order.usdt_amount);
  if (!usdt) return "—";
  return `${(fiat / usdt).toLocaleString(undefined, { maximumFractionDigits: 2 })} ${order.fiat_currency}/USDT`;
}

export default function OrderCard({ order }: { order: Order }) {
  return (
    <Link
      href={`/orders/${order.id}`}
      className="block rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 hover:border-emerald-700"
    >
      <div className="flex items-center justify-between">
        <span className="text-lg font-semibold">
          {parseFloat(order.fiat_amount).toLocaleString()} {order.fiat_currency}
        </span>
        <StatusBadge status={order.status} />
      </div>
      <div className="mt-1 flex justify-between text-sm text-zinc-400">
        <span>{parseFloat(order.usdt_amount).toLocaleString()} USDT · {impliedRate(order)}</span>
        <span>{new Date(order.created_at).toLocaleDateString()}</span>
      </div>
    </Link>
  );
}
```

- [ ] **Step 3: Write new-order page**

`web/src/app/orders/new/page.tsx`:
```tsx
"use client";

import { useEffect, useState } from "react";
import dynamic from "next/dynamic";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { CURRENCIES, type Currency, type Order } from "@/lib/types";

const MapPicker = dynamic(() => import("@/components/MapPicker"), { ssr: false });

// Default: Denpasar, Bali
const DEFAULT_POS = { lat: -8.6705, lng: 115.2126 };

export default function NewOrderPage() {
  const router = useRouter();
  const [currency, setCurrency] = useState<Currency>("IDR");
  const [fiatAmount, setFiatAmount] = useState("");
  const [usdtAmount, setUsdtAmount] = useState("");
  const [address, setAddress] = useState("");
  const [pos, setPos] = useState(DEFAULT_POS);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    navigator.geolocation?.getCurrentPosition(
      (p) => setPos({ lat: p.coords.latitude, lng: p.coords.longitude }),
      () => {},
    );
  }, []);

  const rate =
    parseFloat(fiatAmount) > 0 && parseFloat(usdtAmount) > 0
      ? (parseFloat(fiatAmount) / parseFloat(usdtAmount)).toLocaleString(undefined, { maximumFractionDigits: 2 })
      : null;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const order = await api<Order>("/orders", {
        method: "POST",
        body: JSON.stringify({
          fiat_currency: currency,
          fiat_amount: fiatAmount,
          usdt_amount: usdtAmount,
          address_text: address,
          lat: pos.lat,
          lng: pos.lng,
        }),
      });
      router.push(`/orders/${order.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "failed to create order");
      setSubmitting(false);
    }
  }

  return (
    <div className="mx-auto max-w-lg">
      <h1 className="mb-6 text-2xl font-bold">Get cash delivered</h1>
      <form onSubmit={submit} className="space-y-4">
        <div className="flex gap-3">
          <label className="block w-32">
            <span className="mb-1 block text-sm text-zinc-400">Currency</span>
            <select
              value={currency}
              onChange={(e) => setCurrency(e.target.value as Currency)}
              className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
            >
              {CURRENCIES.map((c) => (
                <option key={c} value={c}>{c}</option>
              ))}
            </select>
          </label>
          <label className="block flex-1">
            <span className="mb-1 block text-sm text-zinc-400">Cash amount you want</span>
            <input
              type="number" step="any" min="0" required
              value={fiatAmount}
              onChange={(e) => setFiatAmount(e.target.value)}
              placeholder="1500000"
              className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
            />
          </label>
        </div>
        <label className="block">
          <span className="mb-1 block text-sm text-zinc-400">USDT you will pay</span>
          <input
            type="number" step="any" min="0" required
            value={usdtAmount}
            onChange={(e) => setUsdtAmount(e.target.value)}
            placeholder="95"
            className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
          />
        </label>
        {rate && <p className="text-sm text-zinc-400">Implied rate: <span className="text-zinc-200">{rate} {currency}/USDT</span> — couriers see this when deciding to accept.</p>}
        <label className="block">
          <span className="mb-1 block text-sm text-zinc-400">Delivery address</span>
          <textarea
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            required maxLength={500} rows={2}
            placeholder="Street, number, notes for the courier..."
            className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2"
          />
        </label>
        <div>
          <span className="mb-1 block text-sm text-zinc-400">Pin your location (drag the pin or click the map)</span>
          <MapPicker value={pos} onChange={setPos} />
        </div>
        {error && <p className="text-sm text-red-400">{error}</p>}
        <button disabled={submitting} className="w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
          {submitting ? "Posting..." : "Post order"}
        </button>
      </form>
    </div>
  );
}
```

- [ ] **Step 4: Write my-orders page**

`web/src/app/orders/page.tsx`:
```tsx
"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { Order } from "@/lib/types";
import OrderCard from "@/components/OrderCard";
import { useUser } from "@/lib/useUser";

const ACTIVE = ["OPEN", "ACCEPTED", "AWAITING_PAYMENT", "PAID"];

export default function OrdersPage() {
  const { user } = useUser();
  const [orders, setOrders] = useState<Order[] | null>(null);

  useEffect(() => {
    if (user) api<Order[]>("/orders/mine").then(setOrders).catch(() => setOrders([]));
  }, [user]);

  if (!orders) return <p className="text-zinc-500">Loading...</p>;

  const active = orders.filter((o) => ACTIVE.includes(o.status));
  const past = orders.filter((o) => !ACTIVE.includes(o.status));

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">My orders</h1>
        <Link href="/orders/new" className="rounded bg-emerald-600 px-4 py-2 text-sm font-medium hover:bg-emerald-500">Get cash</Link>
      </div>
      {orders.length === 0 && <p className="text-zinc-400">No orders yet. Post one to get cash delivered.</p>}
      {active.length > 0 && (
        <section className="space-y-3">
          {active.map((o) => <OrderCard key={o.id} order={o} />)}
        </section>
      )}
      {past.length > 0 && (
        <section className="mt-8">
          <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-zinc-500">Past</h2>
          <div className="space-y-3">{past.map((o) => <OrderCard key={o.id} order={o} />)}</div>
        </section>
      )}
    </div>
  );
}
```

- [ ] **Step 5: Build and commit**

Run: `cd web && npm run build`
Expected: build passes.

```bash
git add web
git commit -m "feat: order creation form with map picker and my-orders list"
```

---

### Task 11: Order detail page, courier board

**Files:**
- Create: `web/src/app/orders/[id]/page.tsx`, `web/src/components/PaymentPanel.tsx`, `web/src/app/courier/page.tsx`

**Interfaces:**
- Consumes: `api`, `types`, `useUser`, `OrderCard`, `StatusBadge`, `impliedRate`; API detail + transition routes from Tasks 6-7.
- Produces: `/orders/[id]` polling every 5s; `<PaymentPanel detail/>` (network tabs + QR + address); `/courier` board with distance and accept.

- [ ] **Step 1: Write PaymentPanel**

`web/src/components/PaymentPanel.tsx`:
```tsx
"use client";

import { useState } from "react";
import { QRCodeSVG } from "qrcode.react";
import type { Network, OrderDetail } from "@/lib/types";

const NETWORK_LABELS: Record<Network, string> = {
  trc20: "TRC20 · Tron",
  bep20: "BEP20 · BNB Chain",
  erc20: "ERC20 · Ethereum",
};

export default function PaymentPanel({ detail }: { detail: OrderDetail }) {
  const usdt = detail.courier_usdt;
  const available = (Object.keys(NETWORK_LABELS) as Network[]).filter((n) => usdt?.[n]);
  const [network, setNetwork] = useState<Network | null>(available[0] ?? null);

  if (!usdt || !network) return null;
  const address = usdt[network]!;

  return (
    <div className="rounded-lg border border-amber-900 bg-amber-950/30 p-4">
      <h3 className="font-semibold text-amber-300">
        Send exactly {parseFloat(detail.usdt_amount).toLocaleString()} USDT
      </h3>
      <p className="mt-1 text-sm text-zinc-400">
        Payment is detected automatically on-chain — usually within a minute of confirmation.
      </p>
      <div className="mt-3 flex gap-2">
        {available.map((n) => (
          <button
            key={n}
            onClick={() => setNetwork(n)}
            className={`rounded px-3 py-1 text-xs font-medium ${n === network ? "bg-amber-600 text-black" : "bg-zinc-800 text-zinc-300"}`}
          >
            {NETWORK_LABELS[n]}
          </button>
        ))}
      </div>
      <div className="mt-4 flex items-center gap-4">
        <div className="rounded bg-white p-2">
          <QRCodeSVG value={address} size={112} />
        </div>
        <div className="min-w-0">
          <p className="text-xs text-zinc-500">Courier&apos;s {NETWORK_LABELS[network]} address</p>
          <p className="break-all font-mono text-sm">{address}</p>
          <button
            onClick={() => navigator.clipboard.writeText(address)}
            className="mt-2 rounded bg-zinc-800 px-3 py-1 text-xs hover:bg-zinc-700"
          >
            Copy address
          </button>
        </div>
      </div>
      <p className="mt-3 animate-pulse text-sm text-amber-400">Waiting for payment on-chain…</p>
    </div>
  );
}
```

- [ ] **Step 2: Write order detail page**

`web/src/app/orders/[id]/page.tsx`:
```tsx
"use client";

import { use, useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { OrderDetail } from "@/lib/types";
import StatusBadge from "@/components/StatusBadge";
import PaymentPanel from "@/components/PaymentPanel";
import { impliedRate } from "@/components/OrderCard";
import { useUser } from "@/lib/useUser";

const STEPS = ["OPEN", "ACCEPTED", "AWAITING_PAYMENT", "PAID", "COMPLETED"] as const;

export default function OrderDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params);
  const { user } = useUser();
  const [detail, setDetail] = useState<OrderDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api<OrderDetail>(`/orders/${id}`).then(setDetail).catch((e: Error) => setError(e.message));
  }, [id]);

  useEffect(() => {
    if (!user) return;
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, [user, load]);

  async function action(name: string) {
    setBusy(true);
    setError(null);
    try {
      await api(`/orders/${id}/${name}`, { method: "POST" });
      load();
    } catch (e) {
      setError(e instanceof Error ? e.message : "action failed");
    } finally {
      setBusy(false);
    }
  }

  if (error && !detail) return <p className="text-red-400">{error}</p>;
  if (!detail) return <p className="text-zinc-500">Loading...</p>;

  const stepIdx = STEPS.indexOf(detail.status as (typeof STEPS)[number]);
  const otherTelegram = detail.is_customer ? detail.courier_telegram : detail.customer_telegram;
  const canCancel =
    (detail.status === "OPEN" && detail.is_customer) ||
    (detail.status === "ACCEPTED" && (detail.is_customer || detail.is_courier)) ||
    (detail.status === "AWAITING_PAYMENT" &&
      (detail.is_customer || detail.is_courier) &&
      detail.payment_requested_at !== null &&
      Date.now() - new Date(detail.payment_requested_at).getTime() > 2 * 3600_000);

  return (
    <div className="mx-auto max-w-lg space-y-5">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">
          {parseFloat(detail.fiat_amount).toLocaleString()} {detail.fiat_currency}
        </h1>
        <StatusBadge status={detail.status} />
      </div>
      <p className="text-zinc-400">
        {parseFloat(detail.usdt_amount).toLocaleString()} USDT · {impliedRate(detail)}
      </p>

      {detail.status !== "CANCELLED" && (
        <ol className="flex gap-1">
          {STEPS.map((s, i) => (
            <li key={s} className={`h-1.5 flex-1 rounded ${i <= stepIdx ? "bg-emerald-500" : "bg-zinc-800"}`} title={s} />
          ))}
        </ol>
      )}

      <div className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 text-sm">
        <p className="text-zinc-400">Delivery address</p>
        <p className="mt-1">{detail.address_text}</p>
      </div>

      {otherTelegram && (
        <a
          href={`https://t.me/${otherTelegram}`}
          target="_blank"
          rel="noreferrer"
          className="block rounded-lg border border-sky-900 bg-sky-950/40 p-4 text-sm hover:border-sky-700"
        >
          💬 Coordinate the meetup on Telegram: <span className="font-medium text-sky-300">@{otherTelegram}</span>
        </a>
      )}

      {detail.is_courier && detail.status === "ACCEPTED" && (
        <button onClick={() => action("request-payment")} disabled={busy}
          className="w-full rounded bg-amber-600 px-4 py-2 font-medium text-black hover:bg-amber-500 disabled:opacity-50">
          I&apos;ve arrived — request USDT payment
        </button>
      )}

      {detail.is_customer && detail.status === "AWAITING_PAYMENT" && <PaymentPanel detail={detail} />}

      {detail.is_courier && detail.status === "AWAITING_PAYMENT" && (
        <p className="animate-pulse text-sm text-amber-400">Waiting for the customer&apos;s USDT to land on-chain…</p>
      )}

      {detail.status === "PAID" && (
        <div className="rounded-lg border border-emerald-900 bg-emerald-950/40 p-4 text-sm">
          <p className="text-emerald-300">✅ USDT received on-chain{detail.payment_txid ? ` (tx ${detail.payment_txid.slice(0, 10)}…)` : ""}.</p>
          {detail.is_courier && <p className="mt-1 text-zinc-300">Hand over the cash now.</p>}
          {detail.is_customer && (
            <button onClick={() => action("confirm-cash")} disabled={busy}
              className="mt-3 w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
              I received the cash
            </button>
          )}
        </div>
      )}

      {detail.status === "COMPLETED" && <p className="text-emerald-400">Order completed. 🎉</p>}
      {detail.status === "CANCELLED" && <p className="text-zinc-400">This order was cancelled.</p>}

      {canCancel && (
        <button onClick={() => action("cancel")} disabled={busy}
          className="w-full rounded border border-red-900 px-4 py-2 text-sm text-red-400 hover:bg-red-950 disabled:opacity-50">
          Cancel order
        </button>
      )}
      {error && <p className="text-sm text-red-400">{error}</p>}
    </div>
  );
}
```

- [ ] **Step 3: Write courier board**

`web/src/app/courier/page.tsx`:
```tsx
"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { Order } from "@/lib/types";
import { impliedRate } from "@/components/OrderCard";
import { useUser } from "@/lib/useUser";

function haversineKm(a: { lat: number; lng: number }, b: { lat: number; lng: number }): number {
  const R = 6371;
  const dLat = ((b.lat - a.lat) * Math.PI) / 180;
  const dLng = ((b.lng - a.lng) * Math.PI) / 180;
  const s =
    Math.sin(dLat / 2) ** 2 +
    Math.cos((a.lat * Math.PI) / 180) * Math.cos((b.lat * Math.PI) / 180) * Math.sin(dLng / 2) ** 2;
  return 2 * R * Math.asin(Math.sqrt(s));
}

export default function CourierPage() {
  const { user } = useUser();
  const [orders, setOrders] = useState<Order[] | null>(null);
  const [me, setMe] = useState<{ lat: number; lng: number } | null>(null);
  const hasAddress = !!(user?.usdt_trc20 || user?.usdt_bep20 || user?.usdt_erc20);

  useEffect(() => {
    if (!user) return;
    const load = () => api<Order[]>("/orders/open").then(setOrders).catch(() => setOrders([]));
    load();
    const t = setInterval(load, 10000);
    navigator.geolocation?.getCurrentPosition((p) => setMe({ lat: p.coords.latitude, lng: p.coords.longitude }), () => {});
    return () => clearInterval(t);
  }, [user]);

  if (!orders) return <p className="text-zinc-500">Loading...</p>;

  return (
    <div>
      <h1 className="mb-2 text-2xl font-bold">Courier board</h1>
      <p className="mb-6 text-sm text-zinc-400">Open cash-delivery requests. Accept one, meet the customer, receive USDT, hand over cash.</p>
      {!hasAddress && (
        <p className="mb-4 rounded bg-amber-950 px-3 py-2 text-sm text-amber-300">
          Add a USDT address in <Link href="/settings" className="underline">settings</Link> before accepting orders.
        </p>
      )}
      {orders.length === 0 && <p className="text-zinc-400">No open orders right now.</p>}
      <div className="space-y-3">
        {orders.map((o) => (
          <Link key={o.id} href={`/orders/${o.id}`}
            className="block rounded-lg border border-zinc-800 bg-zinc-900/50 p-4 hover:border-emerald-700">
            <div className="flex items-center justify-between">
              <span className="text-lg font-semibold">{parseFloat(o.fiat_amount).toLocaleString()} {o.fiat_currency}</span>
              <span className="text-emerald-400">{parseFloat(o.usdt_amount).toLocaleString()} USDT</span>
            </div>
            <div className="mt-1 flex justify-between text-sm text-zinc-400">
              <span>{impliedRate(o)}</span>
              <span>{me ? `${haversineKm(me, o).toFixed(1)} km away` : new Date(o.created_at).toLocaleString()}</span>
            </div>
          </Link>
        ))}
      </div>
    </div>
  );
}
```

Add an "Accept" flow: on the detail page (`/orders/[id]`) an OPEN order viewed by a non-customer shows an accept button. Append to `web/src/app/orders/[id]/page.tsx` before the `canCancel` block in the JSX (after the Telegram link block):
```tsx
      {detail.status === "OPEN" && !detail.is_customer && (
        <button onClick={() => action("accept")} disabled={busy}
          className="w-full rounded bg-emerald-600 px-4 py-2 font-medium hover:bg-emerald-500 disabled:opacity-50">
          Accept this delivery
        </button>
      )}
```

- [ ] **Step 4: Build and commit**

Run: `cd web && npm run build`
Expected: build passes.

```bash
git add web
git commit -m "feat: order detail with payment panel, courier board with accept flow"
```

---

### Task 12: Landing page

**Files:**
- Create: `web/src/app/page.tsx`

**Interfaces:**
- Consumes: `CURRENCIES` from `lib/types`.
- Produces: public landing page at `/`.

- [ ] **Step 1: Write landing page**

`web/src/app/page.tsx`:
```tsx
import Link from "next/link";
import { CURRENCIES } from "@/lib/types";

const STEPS = [
  { title: "Post your order", body: "Say how much cash you need, what you'll pay in USDT, and where you are." },
  { title: "A courier accepts", body: "Couriers see your offer and rate. You coordinate the meetup on Telegram." },
  { title: "Swap at your door", body: "Send USDT to the courier's wallet — we verify it on-chain — and take your cash." },
];

export default function LandingPage() {
  return (
    <div className="py-10">
      <section className="text-center">
        <h1 className="text-4xl font-extrabold tracking-tight">
          Cash, delivered. <span className="text-emerald-400">Paid in USDT.</span>
        </h1>
        <p className="mx-auto mt-4 max-w-xl text-zinc-400">
          peetopee.cash brings physical cash to your door. A courier meets you, you send USDT
          (TRC20, BEP20 or ERC20), the chain confirms it, you get your cash. No bank, no queue.
        </p>
        <div className="mt-8 flex justify-center gap-3">
          <Link href="/orders/new" className="rounded bg-emerald-600 px-6 py-3 font-medium hover:bg-emerald-500">Get cash</Link>
          <Link href="/courier" className="rounded border border-zinc-700 px-6 py-3 font-medium hover:border-emerald-600">Deliver cash</Link>
        </div>
      </section>
      <section className="mx-auto mt-16 grid max-w-2xl gap-6 sm:grid-cols-3">
        {STEPS.map((s, i) => (
          <div key={s.title} className="rounded-lg border border-zinc-800 bg-zinc-900/50 p-5">
            <div className="text-2xl font-bold text-emerald-400">{i + 1}</div>
            <h3 className="mt-2 font-semibold">{s.title}</h3>
            <p className="mt-1 text-sm text-zinc-400">{s.body}</p>
          </div>
        ))}
      </section>
      <section className="mt-16 text-center">
        <p className="text-sm uppercase tracking-wide text-zinc-500">Supported currencies</p>
        <div className="mt-3 flex flex-wrap justify-center gap-2">
          {CURRENCIES.map((c) => (
            <span key={c} className="rounded-full border border-zinc-700 px-3 py-1 text-sm">{c}</span>
          ))}
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 2: Build and commit**

Run: `cd web && npm run build`
Expected: build passes.

```bash
git add web
git commit -m "feat: landing page"
```

---

### Task 13: Docker images, Compose, Caddy, deploy docs

**Files:**
- Create: `api/Dockerfile`, `api/.dockerignore`, `web/Dockerfile`, `web/.dockerignore`, `deploy/caddy/Dockerfile`, `deploy/Caddyfile`, `compose.yml`, `.env.example`, `deploy.sh`, `docs/deploy.md`, `README.md`

**Interfaces:**
- Consumes: everything built in Tasks 1-12.
- Produces: `docker compose up -d` runs the full stack on the droplet; `deploy.sh` redeploys over SSH.

- [ ] **Step 1: Write Dockerfiles**

`api/Dockerfile`:
```dockerfile
FROM rust:1.82 AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src src
COPY migrations migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/peetopee-api /usr/local/bin/peetopee-api
EXPOSE 8080
CMD ["peetopee-api"]
```

`api/.dockerignore`:
```
target
```

`web/Dockerfile`:
```dockerfile
FROM node:22-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:22-alpine AS build
WORKDIR /app
COPY --from=deps /app/node_modules node_modules
COPY . .
RUN npm run build

FROM node:22-alpine
WORKDIR /app
ENV NODE_ENV=production
COPY --from=build /app/.next/standalone ./
COPY --from=build /app/.next/static .next/static
COPY --from=build /app/public public
EXPOSE 3000
CMD ["node", "server.js"]
```

`web/.dockerignore`:
```
node_modules
.next
```

`deploy/caddy/Dockerfile` (Caddy with the Cloudflare DNS plugin for DNS-01 certs behind the proxy):
```dockerfile
FROM caddy:2-builder AS builder
RUN xcaddy build --with github.com/caddy-dns/cloudflare

FROM caddy:2
COPY --from=builder /usr/bin/caddy /usr/bin/caddy
```

- [ ] **Step 2: Write Caddyfile + compose.yml + .env.example**

`deploy/Caddyfile`:
```
peetopee.cash {
	tls {
		dns cloudflare {env.CF_API_TOKEN}
	}
	handle /api/* {
		reverse_proxy api:8080
	}
	handle {
		reverse_proxy web:3000
	}
}
```

`compose.yml` (repo root):
```yaml
services:
  caddy:
    build: ./deploy/caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    environment:
      CF_API_TOKEN: ${CF_API_TOKEN}
    volumes:
      - ./deploy/Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
    depends_on: [web, api]

  web:
    build: ./web
    restart: unless-stopped

  api:
    build: ./api
    restart: unless-stopped
    environment:
      DATABASE_URL: postgres://peetopee:${POSTGRES_PASSWORD}@db:5432/peetopee
      BASE_URL: https://peetopee.cash
      RESEND_API_KEY: ${RESEND_API_KEY}
      ETHERSCAN_API_KEY: ${ETHERSCAN_API_KEY}
      TRONGRID_API_KEY: ${TRONGRID_API_KEY}
    depends_on:
      db:
        condition: service_healthy

  db:
    image: postgres:16
    restart: unless-stopped
    environment:
      POSTGRES_USER: peetopee
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: peetopee
    volumes:
      - pg_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U peetopee"]
      interval: 5s
      timeout: 3s
      retries: 10

volumes:
  caddy_data:
  pg_data:
```

`.env.example`:
```
# copy to .env on the droplet and fill in
POSTGRES_PASSWORD=change-me-long-random
RESEND_API_KEY=re_...
ETHERSCAN_API_KEY=...
TRONGRID_API_KEY=...
CF_API_TOKEN=...   # Cloudflare token with Zone:DNS:Edit on peetopee.cash
```

- [ ] **Step 3: Write deploy.sh + docs/deploy.md + README.md**

`deploy.sh`:
```bash
#!/usr/bin/env bash
# Redeploy peetopee.cash: usage: DROPLET_IP=x.x.x.x ./deploy.sh
set -euo pipefail
: "${DROPLET_IP:?set DROPLET_IP}"
ssh "root@${DROPLET_IP}" 'set -e; cd /opt/peetopee; git pull; docker compose build; docker compose up -d; docker system prune -f'
echo "deployed to https://peetopee.cash"
```
Run `chmod +x deploy.sh`.

`docs/deploy.md`:
```markdown
# Deploying peetopee.cash

## 1. Create the droplet (Bitwyre DO team)

    doctl auth init --context bitwyre       # one-time
    doctl compute ssh-key list --context bitwyre
    doctl compute droplet create peetopee \
      --context bitwyre \
      --region sgp1 \
      --size s-1vcpu-2gb \
      --image ubuntu-24-04-x64 \
      --ssh-keys <YOUR_KEY_ID> \
      --wait
    doctl compute droplet list --context bitwyre --format Name,PublicIPv4

## 2. Cloudflare DNS

- A record: `peetopee.cash` → droplet IP, **proxied** (orange cloud).
- SSL/TLS mode: **Full (strict)**.
- Create an API token: Zone → DNS → Edit, scoped to `peetopee.cash` → goes in `.env` as `CF_API_TOKEN` (used by Caddy for DNS-01 certificates).

## 3. Provision the droplet

    ssh root@<IP>
    apt-get update && apt-get install -y git docker.io docker-compose-v2
    git clone <REPO_URL> /opt/peetopee
    cd /opt/peetopee
    cp .env.example .env && nano .env    # fill all secrets
    docker compose up -d --build

First boot: the API runs sqlx migrations automatically; Caddy fetches certs via
Cloudflare DNS-01.

## 4. Secrets

- `RESEND_API_KEY`: resend.com → verify domain peetopee.cash (DNS records in Cloudflare) → create key.
- `ETHERSCAN_API_KEY`: etherscan.io account → API key (V2 key covers Ethereum + BSC).
- `TRONGRID_API_KEY`: trongrid.io account (optional but avoids rate limits).

## 5. Redeploys

    DROPLET_IP=<IP> ./deploy.sh

## 6. Logs

    ssh root@<IP> 'cd /opt/peetopee && docker compose logs -f --tail 100 api'
```

`README.md`:
```markdown
# peetopee.cash

P2P cash delivery: order cash in your local currency (IDR, AED, EUR, GBP, RUB,
INR, USD, CAD), a courier brings it to you, you pay the courier USDT
(TRC20/BEP20/ERC20) at the meetup — verified on-chain.

## Stack

- `api/` — Rust (Axum + SQLx + Postgres) + USDT chain watcher
- `web/` — Next.js (App Router, Tailwind, Leaflet)
- `deploy/`, `compose.yml` — Caddy + Docker Compose for a single droplet

## Local development

    docker compose -f compose.dev.yml up -d          # Postgres on :5433
    cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo run
    cd web && npm install && npm run dev             # http://localhost:3000, /api proxied to :8080

Without RESEND_API_KEY set, magic-link URLs are printed to the API logs.

    cd api && DATABASE_URL=postgres://peetopee:peetopee@localhost:5433/peetopee cargo test
    cd web && npm run build

## Deploy

See [docs/deploy.md](docs/deploy.md). Spec and plan live in `docs/superpowers/`.
```

- [ ] **Step 4: Verify builds**

Run: `docker compose build`
Expected: all three images build. (Rust release build takes several minutes.)

- [ ] **Step 5: Commit**

```bash
git add api/Dockerfile api/.dockerignore web/Dockerfile web/.dockerignore deploy compose.yml .env.example deploy.sh docs/deploy.md README.md
git commit -m "chore: docker compose deployment with caddy and deploy docs"
```
