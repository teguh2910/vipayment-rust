use std::{env, net::SocketAddr, time::Duration};

/// Configuration for the VIP Reseller provider client. Shared by the services
/// that talk to the provider (catalog sync and fulfillment).
#[derive(Clone, Debug)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub api_id: String,
    pub api_key: String,
    pub sign: Option<String>,
    pub base_url: String,
    pub timeout: Duration,
    pub force_ipv4: bool,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_id: String::new(),
            api_key: String::new(),
            sign: None,
            base_url: vipayment::client::DEFAULT_BASE_URL.to_string(),
            timeout: Duration::from_secs(20),
            force_ipv4: true,
        }
    }
}

impl ProviderConfig {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();

        let enabled = bool_var("PROVIDER_VIPRESELLER_ENABLED", defaults.enabled);
        let api_id = var("PROVIDER_VIPRESELLER_API_ID")
            .or_else(|| var("VIPAYMENT_API_ID"))
            .unwrap_or_default();
        let api_key = var("PROVIDER_VIPRESELLER_API_KEY")
            .or_else(|| var("VIPAYMENT_API_KEY"))
            .unwrap_or_default();

        if enabled && (api_id.is_empty() || api_key.is_empty()) {
            return Err(
                "PROVIDER_VIPRESELLER_API_ID dan PROVIDER_VIPRESELLER_API_KEY wajib diisi"
                    .to_string(),
            );
        }

        Ok(Self {
            enabled,
            api_id,
            api_key,
            sign: var("PROVIDER_VIPRESELLER_SIGN"),
            base_url: var_or("PROVIDER_VIPRESELLER_BASE_URL", &defaults.base_url),
            timeout: Duration::from_secs(parse_num(
                "PROVIDER_TIMEOUT_SECONDS",
                defaults.timeout.as_secs(),
            )?),
            force_ipv4: bool_var("PROVIDER_FORCE_IPV4", defaults.force_ipv4),
        })
    }
}

/// Reads a required environment variable.
pub fn required_var(name: &str) -> Result<String, String> {
    var(name).ok_or_else(|| format!("{name} wajib diisi"))
}

pub fn var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn var_or(name: &str, default: &str) -> String {
    var(name).unwrap_or_else(|| default.to_string())
}

pub fn bind_addr(name: &str, default: &str) -> Result<SocketAddr, String> {
    var_or(name, default)
        .parse::<SocketAddr>()
        .map_err(|e| format!("{name} tidak valid: {e}"))
}

pub fn parse_num<T>(name: &str, default: T) -> Result<T, String>
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

pub fn bool_var(name: &str, default: bool) -> bool {
    match var(name).map(|value| value.to_ascii_lowercase()) {
        Some(value) => matches!(value.as_str(), "true" | "1" | "yes" | "on"),
        None => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_booleans_from_env_style_values() {
        for value in ["true", "1", "yes", "on", "TRUE"] {
            assert!(matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            ));
        }
        for value in ["false", "0", "no", "off", ""] {
            assert!(!matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            ));
        }
    }

    #[test]
    fn provider_config_has_sane_defaults() {
        let cfg = ProviderConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.base_url, vipayment::client::DEFAULT_BASE_URL);
        assert_eq!(cfg.timeout, Duration::from_secs(20));
    }
}
