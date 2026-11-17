pub mod config;
pub mod error;
pub mod routes;

pub use config::Config;
pub use error::AppError;

use std::sync::Arc;
use vipayment::VipaymentClient;

#[derive(Clone)]
pub struct AppState {
    pub client: Arc<VipaymentClient>,
    pub cfg: Arc<Config>,
}

impl AppState {
    pub fn new(cfg: Config) -> Self {
        let mut builder = VipaymentClient::builder(&cfg.api_id, &cfg.api_key);
        if let Some(base_url) = &cfg.base_url {
            builder = builder.base_url(base_url);
        }
        let client = builder.build().expect("Failed to build VipaymentClient");

        Self {
            client: Arc::new(client),
            cfg: Arc::new(cfg),
        }
    }
}
