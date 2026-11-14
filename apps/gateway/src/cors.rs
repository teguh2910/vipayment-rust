use crate::config::Config;
use axum::http::{header, HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Builds the browser CORS policy from the configured origin allowlist.
///
/// A literal `*` allows any origin (convenient for public read-only APIs),
/// otherwise only the listed origins are echoed back. Credentialed requests are
/// never enabled, so `*` does not expose authenticated data to third parties —
/// admin routes still require the bearer token.
pub fn cors_layer(cfg: &Config) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    if cfg.allows_any_origin() {
        return layer.allow_origin(AllowOrigin::any());
    }

    let origins: Vec<HeaderValue> = cfg
        .web_origins
        .iter()
        .filter_map(|origin| match HeaderValue::from_str(origin) {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(origin, "ignoring invalid WEB_ORIGIN entry");
                None
            }
        })
        .collect();

    if origins.is_empty() {
        tracing::warn!("no valid WEB_ORIGIN configured; browser requests will be blocked");
        return layer;
    }

    layer.allow_origin(AllowOrigin::list(origins))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(origins: &[&str]) -> Config {
        Config {
            web_origins: origins.iter().map(|origin| origin.to_string()).collect(),
            ..Config::default()
        }
    }

    #[test]
    fn wildcard_config_reports_any_origin() {
        assert!(config_with(&["*"]).allows_any_origin());
    }

    #[test]
    fn list_config_is_not_wildcard() {
        assert!(!config_with(&["https://a.example.com"]).allows_any_origin());
    }

    #[test]
    fn invalid_origin_entries_are_dropped() {
        // Building the layer must not panic on a malformed origin.
        let _ = cors_layer(&config_with(&["not a valid origin"]));
    }
}
