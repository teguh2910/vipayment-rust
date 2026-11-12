use crate::{config::Config, error::AppError, status::OrderStatus};
use serde_json::Value;
use vipayment::{
    GameOrderData, GameServiceItem, GameStatusItem, PrepaidOrderData, PrepaidServiceItem,
    PrepaidStatusItem, ProfileData, VipaymentClient,
};

#[derive(Debug, Clone)]
pub struct ProviderStatus {
    pub status: Option<String>,
    pub sn: Option<String>,
    pub note: Option<String>,
}

pub struct Provider {
    client: VipaymentClient,
    enabled: bool,
}

impl Provider {
    pub fn new(cfg: &Config) -> Self {
        let mut http = reqwest::Client::builder()
            .user_agent(vipayment::client::DEFAULT_USER_AGENT)
            .timeout(cfg.provider_timeout);

        if cfg.provider_force_ipv4 {
            // VIP Reseller may only allowlist IPv4. Binding the local address to
            // 0.0.0.0 keeps reqwest from preferring the IPv6 route.
            http = http.local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        }

        let http = http.build().expect("failed to build provider HTTP client");

        let client =
            VipaymentClient::builder(cfg.provider_api_id.clone(), cfg.provider_api_key.clone())
                .base_url(cfg.provider_base_url.clone())
                .http_client(http)
                .build()
                .expect("failed to build VIP Reseller client");

        Self {
            client,
            enabled: cfg.provider_enabled,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn signature(&self) -> &str {
        self.client.signature()
    }

    pub fn signature_matches(&self, expected: Option<&str>) -> bool {
        match expected {
            Some(expected) => crate::constant_time_eq(expected, self.client.signature()),
            None => true,
        }
    }

    pub async fn profile(&self) -> Result<ProfileData, AppError> {
        let response = self.client.profile().await?;
        if !response.is_success() {
            return Err(AppError::Provider(response.message));
        }
        response
            .data
            .ok_or_else(|| AppError::Provider("provider tidak mengembalikan data profil".into()))
    }

    pub async fn list_game_services(
        &self,
        game: Option<&str>,
    ) -> Result<Vec<GameServiceItem>, AppError> {
        let response = self.client.service_game(game, Some("available")).await?;
        if !response.is_success() {
            return Err(AppError::Provider(response.message));
        }
        Ok(response.data.unwrap_or_default())
    }

    pub async fn list_prepaid_services(&self) -> Result<Vec<PrepaidServiceItem>, AppError> {
        let response = self.client.service_prepaid(None, None).await?;
        if !response.is_success() {
            return Err(AppError::Provider(response.message));
        }
        Ok(response.data.unwrap_or_default())
    }

    pub async fn create_prepaid_order(
        &self,
        service_code: &str,
        data_no: &str,
    ) -> Result<PrepaidOrderData, AppError> {
        let response = self.client.order_prepaid(service_code, data_no).await?;

        if !response.is_success() {
            return Err(AppError::ProviderRejected(response.message));
        }

        response
            .data
            .ok_or_else(|| AppError::Provider("provider tidak mengembalikan data order".into()))
    }

    pub async fn create_game_order(
        &self,
        service_code: &str,
        data_no: &str,
        data_zone: Option<&str>,
    ) -> Result<GameOrderData, AppError> {
        let response = self
            .client
            .order_game(service_code, data_no, data_zone)
            .await?;

        if !response.is_success() {
            return Err(AppError::ProviderRejected(response.message));
        }

        response
            .data
            .ok_or_else(|| AppError::Provider("provider tidak mengembalikan data order".into()))
    }

    pub async fn game_order_status(
        &self,
        trx_id: &str,
    ) -> Result<Option<ProviderStatus>, AppError> {
        let response = self.client.status_order_game(trx_id, None).await?;
        if !response.is_success() {
            return Err(AppError::Provider(response.message));
        }
        Ok(response
            .data
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(status_from_game))
    }

    pub async fn prepaid_order_status(
        &self,
        trx_id: &str,
    ) -> Result<Option<ProviderStatus>, AppError> {
        let response = self.client.status_order_prepaid(trx_id, None).await?;
        if !response.is_success() {
            return Err(AppError::Provider(response.message));
        }
        Ok(response
            .data
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(status_from_prepaid))
    }
}

fn status_from_game(item: GameStatusItem) -> ProviderStatus {
    ProviderStatus {
        status: item.status,
        sn: item.sn,
        note: item.note,
    }
}

fn status_from_prepaid(item: PrepaidStatusItem) -> ProviderStatus {
    ProviderStatus {
        status: item.status,
        sn: item.sn,
        note: item.note,
    }
}

/// Parses a provider price. VIP Reseller returns prices either as JSON numbers,
/// as strings with thousand separators (e.g. `"5.200"`), or as tier objects
/// such as `{"basic": 1634, "premium": 1626, "special": 1618}`. Tier `basic` is
/// used for objects (the safest tier for a Basic-level account).
pub fn parse_price(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    if let Some(number) = value.as_u64() {
        return i64::try_from(number).ok();
    }
    if let Some(number) = value.as_f64() {
        return Some(number.round() as i64);
    }

    if let Some(object) = value.as_object() {
        for key in ["basic", "premium", "special", "price", "value"] {
            if let Some(inner) = object.get(key) {
                if let Some(parsed) = parse_price(inner) {
                    return Some(parsed);
                }
            }
        }
        return None;
    }

    let raw = value.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }

    // Drop any decimal fraction, then keep digits only so "Rp 5.200" -> 5200.
    let whole = raw.split(',').next().unwrap_or(raw);
    let digits: String = whole.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Applies the configured margin, rounding up to the nearest rupiah.
pub fn markup(base_price: i64, percent: i64) -> i64 {
    if percent <= 0 {
        return base_price;
    }
    (base_price * (100 + percent) + 99) / 100
}

pub fn map_provider_status(raw: &str) -> OrderStatus {
    let status = raw.trim().to_ascii_lowercase();

    if status.contains("success") || status.contains("sukses") || status.contains("berhasil") {
        OrderStatus::Success
    } else if status.contains("fail")
        || status.contains("gagal")
        || status.contains("error")
        || status.contains("cancel")
        || status.contains("batal")
        || status.contains("refund")
    {
        OrderStatus::Failed
    } else {
        OrderStatus::Processing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_prices_in_all_provider_shapes() {
        assert_eq!(parse_price(&json!(5200)), Some(5200));
        assert_eq!(parse_price(&json!(5200.0)), Some(5200));
        assert_eq!(parse_price(&json!("5200")), Some(5200));
        assert_eq!(parse_price(&json!("5.200")), Some(5200));
        assert_eq!(parse_price(&json!("Rp 5.200")), Some(5200));
        assert_eq!(parse_price(&json!("Rp 5.200,00")), Some(5200));
        assert_eq!(
            parse_price(&json!({"basic": 1634, "premium": 1626, "special": 1618})),
            Some(1634)
        );
        assert_eq!(parse_price(&json!(null)), None);
        assert_eq!(parse_price(&json!("-")), None);
    }

    #[test]
    fn applies_markup_with_rounding_up() {
        assert_eq!(markup(10_000, 0), 10_000);
        assert_eq!(markup(10_000, 5), 10_500);
        assert_eq!(markup(1_001, 5), 1_052);
        assert_eq!(markup(21_000, 10), 23_100);
    }

    #[test]
    fn maps_provider_statuses_to_order_statuses() {
        assert_eq!(map_provider_status("success"), OrderStatus::Success);
        assert_eq!(map_provider_status("Sukses"), OrderStatus::Success);
        assert_eq!(map_provider_status("waiting"), OrderStatus::Processing);
        assert_eq!(map_provider_status("processing"), OrderStatus::Processing);
        assert_eq!(map_provider_status("error"), OrderStatus::Failed);
        assert_eq!(map_provider_status("gagal"), OrderStatus::Failed);
        assert_eq!(map_provider_status("canceled"), OrderStatus::Failed);
        assert_eq!(map_provider_status(""), OrderStatus::Processing);
    }
}
