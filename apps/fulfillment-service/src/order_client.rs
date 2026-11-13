use serde::{Deserialize, Serialize};
use std::time::Duration;
use vipayment_shared::AppError;

/// An order claimed from the order service, ready to be submitted to the
/// provider.
#[derive(Debug, Clone, Deserialize)]
pub struct ClaimedOrder {
    pub invoice: String,
    pub kind: String,
    pub product_code: String,
    pub user_id: String,
    pub zone: Option<String>,
}

/// An order already submitted whose provider status should be refreshed.
#[derive(Debug, Clone, Deserialize)]
pub struct PollableOrder {
    pub invoice: String,
    pub kind: String,
    pub provider_trxid: Option<String>,
    pub provider_status: Option<String>,
}

#[derive(Debug, Serialize)]
struct SubmittedBody<'a> {
    provider_trxid: Option<&'a str>,
    provider_status: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct FailureBody<'a> {
    message: &'a str,
}

#[derive(Debug, Serialize)]
struct StatusBody<'a> {
    provider_status: &'a str,
    sn: Option<&'a str>,
    note: Option<&'a str>,
}

/// HTTP client for the order service's internal API. All state transitions stay
/// on the order side; this client only reports outcomes.
#[derive(Clone)]
pub struct OrderClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

impl OrderClient {
    pub fn new(base_url: &str, token: &str, timeout: Duration) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .expect("failed to build order HTTP client");

        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
        }
    }

    /// Claims one paid order. `None` means the queue is empty right now.
    pub async fn claim(&self) -> Result<Option<ClaimedOrder>, AppError> {
        let response = self
            .http
            .post(self.url("/internal/orders/claim"))
            .header(
                vipayment_shared::internal::INTERNAL_TOKEN_HEADER,
                &self.token,
            )
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("order service tidak dapat dihubungi: {error}"))
            })?;

        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "claim gagal ({status}): {body}"
            )));
        }

        serde_json::from_str::<ClaimedOrder>(&body)
            .map(Some)
            .map_err(|error| AppError::Internal(format!("respons claim tidak valid: {error}")))
    }

    pub async fn pollable(&self) -> Result<Vec<PollableOrder>, AppError> {
        #[derive(Deserialize)]
        struct Response {
            orders: Vec<PollableOrder>,
        }

        let body = self.get("/internal/orders/pollable?limit=5").await?;
        let parsed: Response = serde_json::from_str(&body).map_err(|error| {
            AppError::Internal(format!("respons pollable tidak valid: {error}"))
        })?;

        Ok(parsed.orders)
    }

    pub async fn mark_submitted(
        &self,
        invoice: &str,
        provider_trxid: Option<&str>,
        provider_status: Option<&str>,
    ) -> Result<(), AppError> {
        self.post_json(
            &format!("/internal/orders/{}/submitted", encode_segment(invoice)),
            &SubmittedBody {
                provider_trxid,
                provider_status,
            },
        )
        .await
    }

    pub async fn mark_rejected(&self, invoice: &str, message: &str) -> Result<(), AppError> {
        self.post_json(
            &format!("/internal/orders/{}/rejected", encode_segment(invoice)),
            &FailureBody { message },
        )
        .await
    }

    pub async fn mark_failed(&self, invoice: &str, message: &str) -> Result<(), AppError> {
        self.post_json(
            &format!("/internal/orders/{}/failed", encode_segment(invoice)),
            &FailureBody { message },
        )
        .await
    }

    pub async fn report_status(
        &self,
        invoice: &str,
        provider_status: &str,
        sn: Option<&str>,
        note: Option<&str>,
    ) -> Result<(), AppError> {
        self.post_json(
            &format!("/internal/orders/{}/status", encode_segment(invoice)),
            &StatusBody {
                provider_status,
                sn,
                note,
            },
        )
        .await
    }

    async fn get(&self, path: &str) -> Result<String, AppError> {
        let response = self
            .http
            .get(self.url(path))
            .header(
                vipayment_shared::internal::INTERNAL_TOKEN_HEADER,
                &self.token,
            )
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("order service tidak dapat dihubungi: {error}"))
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(AppError::Internal(format!(
                "order service gagal ({status}): {body}"
            )));
        }

        Ok(body)
    }

    async fn post_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<(), AppError> {
        let response = self
            .http
            .post(self.url(path))
            .header(
                vipayment_shared::internal::INTERNAL_TOKEN_HEADER,
                &self.token,
            )
            .json(body)
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("order service tidak dapat dihubungi: {error}"))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "order service menolak laporan ({status}): {body}"
            )));
        }

        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
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
        assert_eq!(encode_segment("INV-260101-ABCDEFGH"), "INV-260101-ABCDEFGH");
        assert_eq!(encode_segment("a/b"), "a%2Fb");
    }

    #[test]
    fn builds_urls_without_double_slashes() {
        let client = OrderClient::new("http://order:8082/", "token", Duration::from_secs(1));
        assert_eq!(
            client.url("/internal/orders/claim"),
            "http://order:8082/internal/orders/claim"
        );
    }
}
