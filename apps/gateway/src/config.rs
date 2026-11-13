use std::net::SocketAddr;
use vipayment_shared::config::{bind_addr, required_var, var, var_or};

#[derive(Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    /// Browser origins allowed to call the API. `vec!["*"]` (or a single `*`
    /// entry in `WEB_ORIGIN`) allows any origin.
    pub web_origins: Vec<String>,
    pub admin_token: String,
    pub catalog_service_url: String,
    pub order_service_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".parse().expect("valid default bind addr"),
            web_origins: vec!["http://localhost:3000".to_string()],
            admin_token: String::new(),
            catalog_service_url: "http://127.0.0.1:8081".to_string(),
            order_service_url: "http://127.0.0.1:8082".to_string(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();

        Ok(Self {
            bind_addr: bind_addr("BIND_ADDR", "127.0.0.1:8080")?,
            web_origins: parse_origins(
                var("WEB_ORIGIN").or_else(|| var("WEB_ORIGINS")).as_deref(),
                &defaults.web_origins,
            )?,
            admin_token: required_var("ADMIN_TOKEN")?,
            catalog_service_url: var_or("CATALOG_SERVICE_URL", &defaults.catalog_service_url),
            order_service_url: var_or("ORDER_SERVICE_URL", &defaults.order_service_url),
        })
    }

    /// Whether every origin is allowed. Reported at startup so an overly open
    /// deployment is obvious in the logs.
    pub fn allows_any_origin(&self) -> bool {
        self.web_origins.iter().any(|origin| origin == "*")
    }
}

/// Parses a comma-separated origin allowlist. Unset falls back to the default
/// single origin; `*` (alone or among others) means "any origin".
fn parse_origins(raw: Option<&str>, defaults: &[String]) -> Result<Vec<String>, String> {
    let Some(raw) = raw else {
        return Ok(defaults.to_vec());
    };

    let origins: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_end_matches('/').to_string())
        .collect();

    if origins.is_empty() {
        return Err("WEB_ORIGIN tidak boleh kosong".to_string());
    }

    Ok(origins)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_route_to_the_two_backends() {
        let cfg = Config::default();
        assert_eq!(cfg.catalog_service_url, "http://127.0.0.1:8081");
        assert_eq!(cfg.order_service_url, "http://127.0.0.1:8082");
        assert_eq!(cfg.bind_addr.port(), 8080);
    }

    #[test]
    fn parses_multiple_origins() {
        let origins =
            parse_origins(Some("https://a.example.com, https://b.example.com/ ,"), &[]).unwrap();

        assert_eq!(
            origins,
            vec!["https://a.example.com", "https://b.example.com"]
        );
    }

    #[test]
    fn wildcard_allows_any_origin() {
        let origins = parse_origins(Some("*"), &[]).unwrap();
        assert_eq!(origins, vec!["*"]);

        let cfg = Config {
            web_origins: origins,
            ..Config::default()
        };
        assert!(cfg.allows_any_origin());
    }

    #[test]
    fn falls_back_to_the_default_origin() {
        let defaults = vec!["http://localhost:3000".to_string()];
        assert_eq!(parse_origins(None, &defaults).unwrap(), defaults);
        assert!(parse_origins(Some(" , "), &defaults).is_err());
    }

    #[test]
    fn single_origin_is_not_wildcard() {
        let cfg = Config::default();
        assert!(!cfg.allows_any_origin());
    }
}
