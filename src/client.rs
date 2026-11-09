//! Asynchronous HTTP client for the VIPayment API.

use crate::error::Result;
use crate::models::{
    ApiResponse, GameOrderData, GameServiceItem, GameStatusItem, PrepaidOrderData,
    PrepaidServiceItem, PrepaidStatusItem, ProfileData,
};
use md5::{Digest, Md5};
use serde::de::DeserializeOwned;
use std::time::Duration;

/// Default VIPayment API base URL.
pub const DEFAULT_BASE_URL: &str = "https://vip-reseller.co.id/api";

/// Default User Agent header.
pub const DEFAULT_USER_AGENT: &str = "VIPayment-Rust-SDK/0.1.0";

/// Computes the MD5 signature for VIPayment: `md5(api_id + api_key)`.
pub fn generate_signature(api_id: &str, api_key: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(format!("{}{}", api_id, api_key).as_bytes());
    hex::encode(hasher.finalize())
}

/// Builder for [`VipaymentClient`].
#[derive(Debug, Clone)]
pub struct VipaymentClientBuilder {
    api_id: String,
    api_key: String,
    base_url: Option<String>,
    timeout: Option<Duration>,
    user_agent: Option<String>,
    http_client: Option<reqwest::Client>,
}

impl VipaymentClientBuilder {
    /// Creates a new builder with the required API ID and API Key.
    pub fn new(api_id: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            api_id: api_id.into(),
            api_key: api_key.into(),
            base_url: None,
            timeout: None,
            user_agent: None,
            http_client: None,
        }
    }

    /// Sets a custom API base URL (useful for proxying or local mock testing).
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Sets the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets a custom User Agent string.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Provides an existing custom [`reqwest::Client`].
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Builds the configured [`VipaymentClient`].
    pub fn build(self) -> Result<VipaymentClient> {
        let base_url = self
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();

        let http = match self.http_client {
            Some(client) => client,
            None => {
                let user_agent = self.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT);

                let mut builder = reqwest::Client::builder().user_agent(user_agent);

                if let Some(t) = self.timeout {
                    builder = builder.timeout(t);
                }

                builder.build()?
            }
        };

        let signature = generate_signature(&self.api_id, &self.api_key);

        Ok(VipaymentClient {
            api_id: self.api_id,
            api_key: self.api_key,
            signature,
            base_url,
            http,
        })
    }
}

/// The main client for interacting with the VIPayment API.
#[derive(Debug, Clone)]
pub struct VipaymentClient {
    api_id: String,
    api_key: String,
    signature: String,
    base_url: String,
    http: reqwest::Client,
}

impl VipaymentClient {
    /// Creates a new `VipaymentClient` with default settings.
    pub fn new(api_id: impl Into<String>, api_key: impl Into<String>) -> Self {
        VipaymentClientBuilder::new(api_id, api_key)
            .build()
            .expect("Failed to build default VipaymentClient")
    }

    /// Creates a builder to configure client options.
    pub fn builder(
        api_id: impl Into<String>,
        api_key: impl Into<String>,
    ) -> VipaymentClientBuilder {
        VipaymentClientBuilder::new(api_id, api_key)
    }

    /// Returns the configured API ID.
    pub fn api_id(&self) -> &str {
        &self.api_id
    }

    /// Returns the configured API Key.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Returns the computed MD5 signature.
    pub fn signature(&self) -> &str {
        &self.signature
    }

    /// Returns the current base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Sends a POST request with URL-encoded form data to the VIPayment API.
    pub async fn post_custom<T: DeserializeOwned>(
        &self,
        endpoint_path: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<ApiResponse<T>> {
        let path = if endpoint_path.starts_with('/') {
            endpoint_path
        } else {
            &format!("/{}", endpoint_path)
        };
        let url = format!("{}{}", self.base_url, path);

        let mut form = Vec::with_capacity(2 + extra_params.len());
        form.push(("key", self.api_key.as_str()));
        form.push(("sign", self.signature.as_str()));

        for (k, v) in extra_params {
            form.push((k, v));
        }

        let resp = self.http.post(&url).form(&form).send().await?;

        let text = resp.text().await?;
        let parsed: ApiResponse<T> = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    /// Retrieves account profile and balance information.
    pub async fn profile(&self) -> Result<ApiResponse<ProfileData>> {
        self.post_custom("/profile", &[]).await
    }

    /// Places a prepaid service order (e.g., PLN token, Pulsa, data packages).
    ///
    /// # Arguments
    /// * `service_code` - Service code (e.g., `"PLN5"`, `"PULSA5"`)
    /// * `data_no` - Target phone number or customer ID
    pub async fn order_prepaid(
        &self,
        service_code: &str,
        data_no: &str,
    ) -> Result<ApiResponse<PrepaidOrderData>> {
        let params = [
            ("type", "order"),
            ("service", service_code),
            ("data_no", data_no),
        ];
        self.post_custom("/prepaid", &params).await
    }

    /// Checks the status of a prepaid order.
    ///
    /// # Arguments
    /// * `trx_id` - Transaction ID
    /// * `limit` - Optional limit on the number of records returned
    pub async fn status_order_prepaid(
        &self,
        trx_id: &str,
        limit: Option<u32>,
    ) -> Result<ApiResponse<Vec<PrepaidStatusItem>>> {
        let limit_str;
        let mut params = vec![("type", "status"), ("trxid", trx_id)];

        if let Some(l) = limit {
            limit_str = l.to_string();
            params.push(("limit", &limit_str));
        }

        self.post_custom("/prepaid", &params).await
    }

    /// Retrieves the list of available prepaid services.
    ///
    /// # Arguments
    /// * `filter_type` - Optional filter category (e.g. `"type"`, `"brand"`)
    /// * `filter_value` - Optional filter value (e.g. `"pulsa-reguler"`, `"telkomsel"`)
    pub async fn service_prepaid(
        &self,
        filter_type: Option<&str>,
        filter_value: Option<&str>,
    ) -> Result<ApiResponse<Vec<PrepaidServiceItem>>> {
        let mut params = vec![("type", "services")];

        if let Some(ft) = filter_type {
            params.push(("filter_type", ft));
        }
        if let Some(fv) = filter_value {
            params.push(("filter_value", fv));
        }

        self.post_custom("/prepaid", &params).await
    }

    /// Places a game/streaming top-up order.
    ///
    /// # Arguments
    /// * `service` - Service code (e.g. `"FF5"`, `"ML5"`)
    /// * `data_no` - Target user ID / account number
    /// * `data_zone` - Optional zone or server ID
    pub async fn order_game(
        &self,
        service: &str,
        data_no: &str,
        data_zone: Option<&str>,
    ) -> Result<ApiResponse<GameOrderData>> {
        let mut params = vec![
            ("type", "order"),
            ("service", service),
            ("data_no", data_no),
        ];

        if let Some(dz) = data_zone {
            params.push(("data_zone", dz));
        }

        self.post_custom("/game-feature", &params).await
    }

    /// Checks the status of a game/streaming order.
    ///
    /// # Arguments
    /// * `trx_id` - Transaction ID
    /// * `limit` - Optional limit
    pub async fn status_order_game(
        &self,
        trx_id: &str,
        limit: Option<u32>,
    ) -> Result<ApiResponse<Vec<GameStatusItem>>> {
        let limit_str;
        let mut params = vec![("type", "status"), ("trxid", trx_id)];

        if let Some(l) = limit {
            limit_str = l.to_string();
            params.push(("limit", &limit_str));
        }

        self.post_custom("/game-feature", &params).await
    }

    /// Retrieves the list of available game and streaming services.
    ///
    /// # Arguments
    /// * `filter_game` - Optional game filter (e.g., `"Mobile Legends"`, `"CapCut Pro"`)
    /// * `filter_status` - Optional status filter (e.g., `"available"`, `"empty"`)
    pub async fn service_game(
        &self,
        filter_game: Option<&str>,
        filter_status: Option<&str>,
    ) -> Result<ApiResponse<Vec<GameServiceItem>>> {
        let mut params = vec![("type", "services")];

        if let Some(fg) = filter_game {
            params.push(("filter_game", fg));
        }
        if let Some(fs) = filter_status {
            params.push(("filter_status", fs));
        }

        self.post_custom("/game-feature", &params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_generation() {
        let api_id = "test_id";
        let api_key = "test_key";
        // md5("test_idtest_key")
        let sign = generate_signature(api_id, api_key);
        assert_eq!(sign, "76e228aaec26d1a075876fbb95dd3c10");
    }

    #[test]
    fn test_client_builder_defaults() {
        let client = VipaymentClient::new("id123", "key456");
        assert_eq!(client.api_id(), "id123");
        assert_eq!(client.api_key(), "key456");
        assert_eq!(client.base_url(), DEFAULT_BASE_URL);
    }
}
