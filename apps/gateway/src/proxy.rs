use crate::{config::Config, routing};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct Gateway {
    pub cfg: Arc<Config>,
    pub http: reqwest::Client,
}

impl Gateway {
    pub fn new(cfg: Arc<Config>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build gateway HTTP client");

        Self { cfg, http }
    }

    /// Proxies an `/api/v1/*` request to the owning service. The public path is
    /// preserved verbatim so clients never learn about the split.
    ///
    /// Admin requests are authenticated here, then the `Authorization` header is
    /// forwarded so the backend enforces the same token again.
    pub async fn proxy_api(State(gateway): State<Gateway>, request: Request) -> Response {
        let path = request.uri().path().to_string();

        let is_admin = path.starts_with("/api/v1/admin/");
        if is_admin && !gateway.admin_authorized(request.headers()) {
            return error_response(StatusCode::UNAUTHORIZED, "unauthorized", "unauthorized");
        }

        let base = match routing::backend_for(strip_api_prefix(&path)) {
            routing::Backend::Catalog => &gateway.cfg.catalog_service_url,
            routing::Backend::Order => &gateway.cfg.order_service_url,
        };

        gateway.forward(base, request, is_admin).await
    }

    /// Proxies `/uploads/*` (order) and `/logos/*` (catalog). Both mount their
    /// static directory at the same public prefix the monolith exposed.
    pub async fn proxy_static(State(gateway): State<Gateway>, request: Request) -> Response {
        let path = request.uri().path().to_string();
        let base = if path.starts_with("/logos/") {
            &gateway.cfg.catalog_service_url
        } else {
            &gateway.cfg.order_service_url
        };

        gateway.forward(base, request, false).await
    }

    /// Aggregates both health documents behind the single public endpoint.
    pub async fn health(State(gateway): State<Gateway>) -> Response {
        let catalog = gateway.fetch_health(&gateway.cfg.catalog_service_url).await;
        let order = gateway.fetch_health(&gateway.cfg.order_service_url).await;

        axum::Json(routing::merge_health(catalog, order)).into_response()
    }

    fn admin_authorized(&self, headers: &HeaderMap) -> bool {
        let provided = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .unwrap_or_default();

        vipayment_shared::constant_time_eq(provided, &self.cfg.admin_token)
    }

    async fn fetch_health(&self, base: &str) -> Option<Value> {
        let url = format!("{}/api/v1/health", base.trim_end_matches('/'));

        match self.http.get(url).send().await {
            Ok(response) if response.status().is_success() => response.json::<Value>().await.ok(),
            Ok(response) => {
                tracing::warn!(%base, status = %response.status(), "health upstream returned an error");
                None
            }
            Err(error) => {
                tracing::warn!(%base, %error, "health upstream unreachable");
                None
            }
        }
    }

    async fn forward(&self, base: &str, request: Request, keep_auth: bool) -> Response {
        let (parts, body) = request.into_parts();

        let body = match axum::body::to_bytes(body, 4 * 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(%error, "failed to read request body");
                return error_response(StatusCode::BAD_REQUEST, "bad_request", "body tidak valid");
            }
        };

        let url = build_url(base, &parts.uri);
        let method = reqwest_method(&parts.method);

        let mut outbound = self
            .http
            .request(method, &url)
            .headers(forward_headers(&parts.headers, keep_auth));

        if !body.is_empty() {
            outbound = outbound.body(body);
        }

        let response = match outbound.send().await {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(%url, %error, "upstream request failed");
                return error_response(
                    StatusCode::BAD_GATEWAY,
                    "upstream_error",
                    "layanan tidak tersedia",
                );
            }
        };

        relay(response).await
    }
}

fn strip_api_prefix(path: &str) -> &str {
    path.strip_prefix("/api/v1").unwrap_or(path)
}

fn build_url(base: &str, uri: &Uri) -> String {
    let mut url = format!("{}{}", base.trim_end_matches('/'), uri.path());

    if let Some(query) = uri.query() {
        url.push('?');
        url.push_str(query);
    }

    url
}

fn reqwest_method(method: &axum::http::Method) -> reqwest::Method {
    reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET)
}

/// Forwards only the headers a backend needs. Hop-by-hop headers are dropped;
/// `Authorization` survives only on admin requests that the edge already
/// validated.
fn forward_headers(headers: &HeaderMap, keep_auth: bool) -> HeaderMap {
    let mut out = HeaderMap::new();

    for name in [
        header::CONTENT_TYPE,
        header::ACCEPT,
        header::USER_AGENT,
        header::IF_NONE_MATCH,
        header::IF_MODIFIED_SINCE,
    ] {
        if let Some(value) = headers.get(&name) {
            out.insert(name, value.clone());
        }
    }

    if keep_auth {
        if let Some(value) = headers.get(header::AUTHORIZATION) {
            out.insert(header::AUTHORIZATION, value.clone());
        }
    }

    out
}

async fn relay(response: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();

    match response.bytes().await {
        Ok(bytes) => {
            let mut out = bytes.into_response();
            *out.status_mut() = status;
            if let Some(content_type) = content_type {
                out.headers_mut().insert(header::CONTENT_TYPE, content_type);
            }
            out
        }
        Err(error) => {
            tracing::warn!(%error, "failed to read upstream body");
            error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                "respons layanan tidak valid",
            )
        }
    }
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        axum::Json(json!({ "error": code, "message": message })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls_with_query_strings() {
        let uri: Uri = "/api/v1/orders?status=paid&q=abc".parse().unwrap();
        assert_eq!(
            build_url("http://order:8082", &uri),
            "http://order:8082/api/v1/orders?status=paid&q=abc"
        );
    }

    #[test]
    fn strips_trailing_slashes_from_bases() {
        let uri: Uri = "/api/v1/games".parse().unwrap();
        assert_eq!(
            build_url("http://catalog:8081/", &uri),
            "http://catalog:8081/api/v1/games"
        );
    }

    #[test]
    fn drops_authorization_unless_keeping_admin_auth() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());

        let public = forward_headers(&headers, false);
        assert!(public.get(header::AUTHORIZATION).is_none());

        let admin = forward_headers(&headers, true);
        assert_eq!(admin.get(header::AUTHORIZATION).unwrap(), "Bearer secret");
    }
}
