use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub api_id: String,
    pub api_key: String,
    pub base_url: Option<String>,
    pub cors_origin: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let api_id = env::var("VIPAYMENT_API_ID")
            .map_err(|_| "VIPAYMENT_API_ID environment variable is required".to_string())?;
        let api_key = env::var("VIPAYMENT_API_KEY")
            .map_err(|_| "VIPAYMENT_API_KEY environment variable is required".to_string())?;
        let base_url = env::var("VIPAYMENT_BASE_URL").ok();
        let cors_origin = env::var("CORS_ORIGIN").unwrap_or_else(|_| "*".to_string());

        Ok(Self {
            bind_addr,
            api_id,
            api_key,
            base_url,
            cors_origin,
        })
    }
}
