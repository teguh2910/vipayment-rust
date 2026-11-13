use serde::Deserialize;
use std::time::Duration;
use vipayment_shared::AppError;

/// Group metadata and product price resolved from the catalog service at
/// checkout. Mirrors `/internal/catalog/resolve/{slug}/{code}`.
#[derive(Debug, Clone, Deserialize)]
pub struct ResolvedProduct {
    pub game: ResolvedGame,
    pub product: ResolvedProductInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolvedGame {
    pub slug: String,
    pub name: String,
    pub icon_url: Option<String>,
    pub requires_zone: bool,
    pub user_id_label: String,
    pub zone_label: String,
    pub kind: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolvedProductInfo {
    pub code: String,
    pub name: String,
    pub base_price: i64,
    pub sell_price: i64,
}

#[derive(Clone)]
pub struct CatalogClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl CatalogClient {
    pub fn new(base_url: &str, token: &str, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("failed to build catalog HTTP client");

        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    /// Resolves a product for checkout. A missing or hidden product surfaces as
    /// `NotFound`, an unavailable one as `Conflict`, matching the old in-process
    /// behavior.
    pub async fn resolve(
        &self,
        game_slug: &str,
        product_code: &str,
    ) -> Result<ResolvedProduct, AppError> {
        let url = format!(
            "{}/internal/catalog/resolve/{}/{}",
            self.base_url,
            encode_segment(game_slug),
            encode_segment(product_code)
        );

        let response = self
            .http
            .get(url)
            .header(
                vipayment_shared::internal::INTERNAL_TOKEN_HEADER,
                &self.token,
            )
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("catalog service tidak dapat dihubungi: {error}"))
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status.is_success() {
            return serde_json::from_str::<ResolvedProduct>(&body).map_err(|error| {
                AppError::Internal(format!("respons catalog tidak valid: {error}"))
            });
        }

        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(|message| message.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "katalog tidak tersedia".to_string());

        match status.as_u16() {
            404 => Err(AppError::NotFound(message)),
            409 => Err(AppError::Conflict(message)),
            _ => Err(AppError::Internal(format!(
                "catalog service gagal ({status}): {message}"
            ))),
        }
    }
}

fn encode_segment(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_path_segments() {
        assert_eq!(encode_segment("mobile-legends"), "mobile-legends");
        assert_eq!(encode_segment("a/b c"), "a%2Fb%20c");
    }
}
