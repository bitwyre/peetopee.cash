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
