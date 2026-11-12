use std::{env, net::SocketAddr, time::Duration};

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
    pub web_origin: String,
    pub admin_token: String,
    pub provider_enabled: bool,
    pub provider_api_id: String,
    pub provider_api_key: String,
    pub provider_sign: Option<String>,
    pub provider_base_url: String,
    pub provider_timeout: Duration,
    pub provider_force_ipv4: bool,
    pub payment_provider: PaymentProviderKind,
    pub payment_expiry_minutes: i64,
    pub markup_percent: i64,
    pub simulation_auto_pay_seconds: i64,
    pub max_order_amount: i64,
    pub manual_qris_image_url: Option<String>,
    pub manual_bank_accounts: Vec<BankAccount>,
    pub upload_dir: String,
    pub games_config_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".parse().expect("valid default bind addr"),
            database_url: "sqlite://data/app.db".to_string(),
            web_origin: "http://localhost:3000".to_string(),
            admin_token: "dev-admin-token".to_string(),
            provider_enabled: true,
            provider_api_id: String::new(),
            provider_api_key: String::new(),
            provider_sign: None,
            provider_base_url: vipayment::client::DEFAULT_BASE_URL.to_string(),
            provider_timeout: Duration::from_secs(20),
            provider_force_ipv4: true,
            payment_provider: PaymentProviderKind::Manual,
            payment_expiry_minutes: 1440,
            markup_percent: 5,
            simulation_auto_pay_seconds: 5,
            max_order_amount: 10_000_000,
            manual_qris_image_url: None,
            manual_bank_accounts: Vec::new(),
            upload_dir: "data/uploads".to_string(),
            games_config_path: "config/games.json".to_string(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();

        let provider_enabled = bool_var("PROVIDER_VIPRESELLER_ENABLED", defaults.provider_enabled);
        let provider_api_id = var("PROVIDER_VIPRESELLER_API_ID")
            .or_else(|| var("VIPAYMENT_API_ID"))
            .unwrap_or_default();
        let provider_api_key = var("PROVIDER_VIPRESELLER_API_KEY")
            .or_else(|| var("VIPAYMENT_API_KEY"))
            .unwrap_or_default();

        if provider_enabled && (provider_api_id.is_empty() || provider_api_key.is_empty()) {
            return Err(
                "PROVIDER_VIPRESELLER_API_ID dan PROVIDER_VIPRESELLER_API_KEY wajib diisi"
                    .to_string(),
            );
        }

        let admin_token =
            var("ADMIN_TOKEN").ok_or_else(|| "ADMIN_TOKEN wajib diisi".to_string())?;

        let bind_addr = var_or("BIND_ADDR", "127.0.0.1:8080")
            .parse::<SocketAddr>()
            .map_err(|e| format!("BIND_ADDR tidak valid: {e}"))?;

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
            bind_addr,
            database_url: var_or("DATABASE_URL", &defaults.database_url),
            web_origin: var_or("WEB_ORIGIN", &defaults.web_origin),
            admin_token,
            provider_enabled,
            provider_api_id,
            provider_api_key,
            provider_sign: var("PROVIDER_VIPRESELLER_SIGN"),
            provider_base_url: var_or("PROVIDER_VIPRESELLER_BASE_URL", &defaults.provider_base_url),
            provider_timeout: Duration::from_secs(parse_num(
                "PROVIDER_TIMEOUT_SECONDS",
                defaults.provider_timeout.as_secs(),
            )?),
            provider_force_ipv4: bool_var("PROVIDER_FORCE_IPV4", defaults.provider_force_ipv4),
            payment_provider,
            payment_expiry_minutes: parse_num(
                "PAYMENT_EXPIRY_MINUTES",
                defaults.payment_expiry_minutes,
            )?,
            markup_percent: parse_num("MARKUP_PERCENT", defaults.markup_percent)?,
            simulation_auto_pay_seconds: parse_num(
                "SIMULATION_AUTO_PAY_SECONDS",
                defaults.simulation_auto_pay_seconds,
            )?,
            max_order_amount: parse_num("MAX_ORDER_AMOUNT", defaults.max_order_amount)?,
            manual_qris_image_url: var("MANUAL_QRIS_IMAGE_URL"),
            manual_bank_accounts,
            upload_dir: var_or("UPLOAD_DIR", &defaults.upload_dir),
            games_config_path: var_or("GAMES_CONFIG_PATH", &defaults.games_config_path),
        })
    }
}

fn var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn var_or(name: &str, default: &str) -> String {
    var(name).unwrap_or_else(|| default.to_string())
}

fn parse_num<T>(name: &str, default: T) -> Result<T, String>
where
    T: std::str::FromStr + Copy,
{
    match var(name) {
        Some(raw) => raw
            .parse::<T>()
            .map_err(|_| format!("{name} tidak valid: {raw}")),
        None => Ok(default),
    }
}

fn bool_var(name: &str, default: bool) -> bool {
    match var(name).map(|value| value.to_ascii_lowercase()) {
        Some(value) => matches!(value.as_str(), "true" | "1" | "yes" | "on"),
        None => default,
    }
}
