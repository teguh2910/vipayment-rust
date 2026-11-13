use std::time::Duration;
use vipayment_shared::config::{bool_var, parse_num, required_var, var_or, ProviderConfig};

#[derive(Clone)]
pub struct Config {
    pub order_service_url: String,
    pub internal_token: String,
    pub poll_interval: Duration,
    pub enabled: bool,
    pub provider: ProviderConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            order_service_url: "http://127.0.0.1:8082".to_string(),
            internal_token: String::new(),
            poll_interval: Duration::from_secs(10),
            enabled: true,
            provider: ProviderConfig::default(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();

        let internal_token = required_var("INTERNAL_TOKEN")?;
        if internal_token.len() < 16 {
            return Err("INTERNAL_TOKEN minimal 16 karakter".to_string());
        }

        Ok(Self {
            order_service_url: var_or("ORDER_SERVICE_URL", &defaults.order_service_url),
            internal_token,
            poll_interval: Duration::from_secs(parse_num(
                "POLL_INTERVAL_SECONDS",
                defaults.poll_interval.as_secs(),
            )?),
            enabled: bool_var("FULFILLMENT_ENABLED", defaults.enabled),
            provider: ProviderConfig::from_env()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_target_the_order_service() {
        let cfg = Config::default();
        assert_eq!(cfg.order_service_url, "http://127.0.0.1:8082");
        assert_eq!(cfg.poll_interval, Duration::from_secs(10));
        assert!(cfg.enabled);
    }
}
