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
    pub ip_limiter: Arc<RateLimiter>,
}
