//! CORS behavior for third-party frontends: an allowed origin gets the
//! preflight approved, an unlisted one does not.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    routing::get,
    Router,
};
use std::sync::Arc;
use tower::ServiceExt;
use vipayment_gateway::{config::Config, cors::cors_layer, proxy::Gateway};
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

fn config_with(origins: &[&str]) -> Config {
    Config {
        web_origins: origins.iter().map(|origin| origin.to_string()).collect(),
        admin_token: "test-admin-token".to_string(),
        ..Config::default()
    }
}

fn app(cfg: Config) -> Router {
    Router::new()
        .route("/api/v1/games", get(|| async { "ok" }))
        .layer(cors_layer(&cfg))
}

fn preflight(origin: &str) -> Request<Body> {
    Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/games")
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
        .body(Body::empty())
        .unwrap()
}

fn allow_origin(response: &axum::response::Response) -> Option<String> {
    response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

#[tokio::test]
async fn preflight_from_a_listed_origin_is_approved() {
    let app = app(config_with(&[
        "https://shop.example.com",
        "https://admin.example.com",
    ]));

    let response = app
        .oneshot(preflight("https://shop.example.com"))
        .await
        .unwrap();

    assert!(response.status().is_success());
    assert_eq!(
        allow_origin(&response).as_deref(),
        Some("https://shop.example.com")
    );
}

#[tokio::test]
async fn preflight_from_an_unlisted_origin_is_not_approved() {
    let app = app(config_with(&["https://shop.example.com"]));

    let response = app
        .oneshot(preflight("https://evil.example.com"))
        .await
        .unwrap();

    // The layer answers the preflight but without an allow-origin header, so the
    // browser blocks the real request.
    assert!(allow_origin(&response).is_none());
}

#[tokio::test]
async fn wildcard_allows_any_origin() {
    let app = app(config_with(&["*"]));

    let response = app
        .oneshot(preflight("https://anything.example.com"))
        .await
        .unwrap();

    assert!(response.status().is_success());
    assert_eq!(allow_origin(&response).as_deref(), Some("*"));
}

/// End-to-end: a browser on another site can read the catalog through the
/// gateway when its origin is allowlisted.
#[tokio::test]
async fn third_party_frontend_can_read_catalog_through_the_gateway() {
    let catalog = MockServer::start().await;
    let order = MockServer::start().await;

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/api/v1/games"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "games": [{ "slug": "mobile-legends" }]
        })))
        .mount(&catalog)
        .await;

    let cfg = Config {
        catalog_service_url: catalog.uri(),
        order_service_url: order.uri(),
        web_origins: vec!["https://shop.example.com".to_string()],
        ..Config::default()
    };

    let app = Router::new()
        .route("/api/v1/{*path}", axum::routing::any(Gateway::proxy_api))
        .layer(cors_layer(&cfg))
        .with_state(Gateway::new(Arc::new(cfg)));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/games")
                .header(header::ORIGIN, "https://shop.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        allow_origin(&response).as_deref(),
        Some("https://shop.example.com")
    );
}
