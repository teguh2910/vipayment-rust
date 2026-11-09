//! Synchronous (blocking) HTTP client for the VIPayment API.
//!
//! Enabled via the `blocking` Cargo feature.

use crate::client::{generate_signature, DEFAULT_BASE_URL, DEFAULT_USER_AGENT};
use crate::error::Result;
use crate::models::{
    ApiResponse, GameOrderData, GameServiceItem, GameStatusItem, PrepaidOrderData,
    PrepaidServiceItem, PrepaidStatusItem, ProfileData,
};
use serde::de::DeserializeOwned;
use std::time::Duration;

/// Builder for [`BlockingVipaymentClient`].
#[derive(Debug, Clone)]
pub struct BlockingVipaymentClientBuilder {
    api_id: String,
    api_key: String,
    base_url: Option<String>,
    timeout: Option<Duration>,
    user_agent: Option<String>,
    http_client: Option<reqwest::blocking::Client>,
}

impl BlockingVipaymentClientBuilder {
    /// Creates a new builder with required credentials.
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

    /// Sets a custom API base URL.
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

    /// Provides an existing custom [`reqwest::blocking::Client`].
    pub fn http_client(mut self, client: reqwest::blocking::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Builds the configured [`BlockingVipaymentClient`].
    pub fn build(self) -> Result<BlockingVipaymentClient> {
        let base_url = self
            .base_url
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();

        let http = match self.http_client {
            Some(client) => client,
            None => {
                let user_agent = self.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT);

                let mut builder = reqwest::blocking::Client::builder().user_agent(user_agent);

                if let Some(t) = self.timeout {
                    builder = builder.timeout(t);
                }

                builder.build()?
            }
        };

        let signature = generate_signature(&self.api_id, &self.api_key);

        Ok(BlockingVipaymentClient {
            api_id: self.api_id,
            api_key: self.api_key,
            signature,
            base_url,
            http,
        })
    }
}

/// Synchronous client for interacting with the VIPayment API.
#[derive(Debug, Clone)]
pub struct BlockingVipaymentClient {
    api_id: String,
    api_key: String,
    signature: String,
    base_url: String,
    http: reqwest::blocking::Client,
}

impl BlockingVipaymentClient {
    /// Creates a new `BlockingVipaymentClient` with default settings.
    pub fn new(api_id: impl Into<String>, api_key: impl Into<String>) -> Self {
        BlockingVipaymentClientBuilder::new(api_id, api_key)
            .build()
            .expect("Failed to build default BlockingVipaymentClient")
    }

    /// Creates a builder for custom client options.
    pub fn builder(
        api_id: impl Into<String>,
        api_key: impl Into<String>,
    ) -> BlockingVipaymentClientBuilder {
        BlockingVipaymentClientBuilder::new(api_id, api_key)
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

    /// Sends a POST request with form URL-encoded data synchronously.
    pub fn post_custom<T: DeserializeOwned>(
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

        let resp = self.http.post(&url).form(&form).send()?;
        let text = resp.text()?;
        let parsed: ApiResponse<T> = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    /// Retrieves account profile information.
    pub fn profile(&self) -> Result<ApiResponse<ProfileData>> {
        self.post_custom("/profile", &[])
    }

    /// Places a prepaid service order.
    pub fn order_prepaid(
        &self,
        service_code: &str,
        data_no: &str,
    ) -> Result<ApiResponse<PrepaidOrderData>> {
        let params = [
            ("type", "order"),
            ("service", service_code),
            ("data_no", data_no),
        ];
        self.post_custom("/prepaid", &params)
    }

    /// Checks the status of a prepaid order.
    pub fn status_order_prepaid(
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

        self.post_custom("/prepaid", &params)
    }

    /// Retrieves the list of available prepaid services.
    pub fn service_prepaid(
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

        self.post_custom("/prepaid", &params)
    }

    /// Places a game/streaming top-up order.
    pub fn order_game(
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

        self.post_custom("/game-feature", &params)
    }

    /// Checks the status of a game/streaming order.
    pub fn status_order_game(
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

        self.post_custom("/game-feature", &params)
    }

    /// Retrieves the list of available game and streaming services.
    pub fn service_game(
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

        self.post_custom("/game-feature", &params)
    }
}
