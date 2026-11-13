use std::{net::SocketAddr, time::Duration};
use vipayment_shared::config::{bind_addr, parse_num, required_var, var, var_or};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentProviderKind {
    Manual,
    Simulation,
}

impl PaymentProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Simulation => "simulation",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "manual" => Ok(Self::Manual),
            "simulation" | "sim" => Ok(Self::Simulation),
            other => Err(format!(
                "PAYMENT_PROVIDER tidak dikenal: {other} (pilih manual atau simulation)"
            )),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BankAccount {
    pub bank: String,
    pub account_no: String,
    pub account_name: String,
}

#[derive(Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub payment_provider: PaymentProviderKind,
    pub payment_expiry_minutes: i64,
    pub simulation_auto_pay_seconds: i64,
    pub max_order_amount: i64,
    pub manual_qris_image_url: Option<String>,
    pub manual_bank_accounts: Vec<BankAccount>,
    pub upload_dir: String,
    pub catalog_service_url: String,
    pub internal_token: String,
    pub admin_token: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8082".parse().expect("valid default bind addr"),
            database_url: "sqlite://data/orders.db".to_string(),
            payment_provider: PaymentProviderKind::Manual,
            payment_expiry_minutes: 1440,
            simulation_auto_pay_seconds: 5,
            max_order_amount: 10_000_000,
            manual_qris_image_url: None,
            manual_bank_accounts: Vec::new(),
            upload_dir: "data/uploads".to_string(),
            catalog_service_url: "http://127.0.0.1:8081".to_string(),
            internal_token: String::new(),
            admin_token: String::new(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();

        let admin_token = required_var("ADMIN_TOKEN")?;
        let internal_token = required_var("INTERNAL_TOKEN")?;
        if internal_token.len() < 16 {
            return Err("INTERNAL_TOKEN minimal 16 karakter".to_string());
        }

        let payment_provider = PaymentProviderKind::parse(&var_or(
            "PAYMENT_PROVIDER",
            defaults.payment_provider.as_str(),
        ))?;

        let manual_bank_accounts = match var("MANUAL_BANK_ACCOUNTS") {
            Some(raw) => serde_json::from_str::<Vec<BankAccount>>(&raw)
                .map_err(|e| format!("MANUAL_BANK_ACCOUNTS bukan JSON yang valid: {e}"))?,
            None => Vec::new(),
        };

        Ok(Self {
            bind_addr: bind_addr("BIND_ADDR", "127.0.0.1:8082")?,
            database_url: var_or("DATABASE_URL", &defaults.database_url),
            payment_provider,
            payment_expiry_minutes: parse_num(
                "PAYMENT_EXPIRY_MINUTES",
                defaults.payment_expiry_minutes,
            )?,
            simulation_auto_pay_seconds: parse_num(
                "SIMULATION_AUTO_PAY_SECONDS",
                defaults.simulation_auto_pay_seconds,
            )?,
            max_order_amount: parse_num("MAX_ORDER_AMOUNT", defaults.max_order_amount)?,
            manual_qris_image_url: var("MANUAL_QRIS_IMAGE_URL"),
            manual_bank_accounts,
            upload_dir: var_or("UPLOAD_DIR", &defaults.upload_dir),
            catalog_service_url: var_or("CATALOG_SERVICE_URL", &defaults.catalog_service_url),
            internal_token,
            admin_token,
        })
    }

    pub fn catalog_timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_payment_provider_kinds() {
        assert_eq!(
            PaymentProviderKind::parse("manual").unwrap(),
            PaymentProviderKind::Manual
        );
        assert_eq!(
            PaymentProviderKind::parse(" Simulation ").unwrap(),
            PaymentProviderKind::Simulation
        );
        assert!(PaymentProviderKind::parse("midtrans").is_err());
    }

    #[test]
    fn config_defaults_point_at_other_services() {
        let cfg = Config::default();
        assert_eq!(cfg.catalog_service_url, "http://127.0.0.1:8081");
        assert_eq!(cfg.bind_addr.port(), 8082);
    }
}
