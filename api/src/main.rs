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
        Some(key) => Arc::new(ResendMailer {
            api_key: key.clone(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("http client"),
        }),
        None => Arc::new(LogMailer),
    };
    let state = AppState {
        pool,
        config: Arc::new(config.clone()),
        mailer,
        limiter: Arc::new(RateLimiter::new(3, Duration::from_secs(900))),
        ip_limiter: Arc::new(RateLimiter::new(30, Duration::from_secs(900))),
    };
    tokio::spawn(peetopee_api::watcher::run(state.clone()));
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await.unwrap();
    tracing::info!("listening on {}", config.bind_addr);
    axum::serve(listener, peetopee_api::app(state)).await.unwrap();
}
