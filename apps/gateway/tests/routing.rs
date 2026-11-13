use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;
use vipayment_gateway::{config::Config, proxy::Gateway};
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

fn gateway_config(catalog: String, order: String) -> Config {
    Config {
        web_origins: vec!["http://localhost:3000".to_string()],
        admin_token: "test-admin-token".to_string(),
        catalog_service_url: catalog,
        order_service_url: order,
        ..Config::default()
    }
}

fn router(gateway: Gateway) -> axum::Router {
    use axum::routing::{any, get};

    axum::Router::new()
        .route("/api/v1/health", get(Gateway::health))
        .route("/api/v1/{*path}", any(Gateway::proxy_api))
        .route("/uploads/{*path}", any(Gateway::proxy_static))
        .route("/logos/{*path}", any(Gateway::proxy_static))
        .with_state(gateway)
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn routes_games_to_the_catalog_service() {
    let catalog = MockServer::start().await;
    let order = MockServer::start().await;

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/api/v1/games"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "games": [{ "slug": "mobile-legends" }]
        })))
        .mount(&catalog)
        .await;

    let app = router(Gateway::new(Arc::new(gateway_config(
        catalog.uri(),
        order.uri(),
    ))));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/games")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["games"][0]["slug"], "mobile-legends");

    // The order service must not have been touched.
    assert!(order.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn routes_orders_to_the_order_service() {
    let catalog = MockServer::start().await;
    let order = MockServer::start().await;

    Mock::given(method("POST"))
        .and(wiremock::matchers::path("/api/v1/orders"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "invoice": "INV-1",
            "status": "pending_payment"
        })))
        .mount(&order)
        .await;

    let app = router(Gateway::new(Arc::new(gateway_config(
        catalog.uri(),
        order.uri(),
    ))));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"game_slug":"mobile-legends"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    assert_eq!(body["invoice"], "INV-1");
    assert!(catalog.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn admin_requests_without_a_token_are_rejected_at_the_edge() {
    let catalog = MockServer::start().await;
    let order = MockServer::start().await;

    let app = router(Gateway::new(Arc::new(gateway_config(
        catalog.uri(),
        order.uri(),
    ))));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/orders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(order.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn admin_requests_with_a_token_reach_the_backend() {
    let catalog = MockServer::start().await;
    let order = MockServer::start().await;

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/api/v1/admin/orders"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "orders": [] })))
        .mount(&order)
        .await;

    let app = router(Gateway::new(Arc::new(gateway_config(
        catalog.uri(),
        order.uri(),
    ))));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/orders")
                .header("authorization", "Bearer test-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let requests = order.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    // The backend enforces the token again, so it must be forwarded.
    assert_eq!(
        requests[0]
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer test-admin-token")
    );
}

#[tokio::test]
async fn health_endpoint_merges_both_services() {
    let catalog = MockServer::start().await;
    let order = MockServer::start().await;

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/api/v1/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "last_sync_at": "2026-01-01T00:00:00Z",
            "logos_cached": 56
        })))
        .mount(&catalog)
        .await;

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/api/v1/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ok": true,
            "payment_provider": "manual"
        })))
        .mount(&order)
        .await;

    let app = router(Gateway::new(Arc::new(gateway_config(
        catalog.uri(),
        order.uri(),
    ))));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["logos_cached"], 56);
    assert_eq!(body["payment_provider"], "manual");
    assert_eq!(body["catalog_service_ok"], true);
    assert_eq!(body["order_service_ok"], true);
}

#[tokio::test]
async fn logos_and_uploads_go_to_their_own_services() {
    let catalog = MockServer::start().await;
    let order = MockServer::start().await;

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/logos/ml.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"png".to_vec()))
        .mount(&catalog)
        .await;

    Mock::given(method("GET"))
        .and(wiremock::matchers::path("/uploads/proof.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"proof".to_vec()))
        .mount(&order)
        .await;

    let gateway = Gateway::new(Arc::new(gateway_config(catalog.uri(), order.uri())));

    let logo = router(gateway.clone())
        .oneshot(
            Request::builder()
                .uri("/logos/ml.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logo.status(), StatusCode::OK);

    let proof = router(gateway)
        .oneshot(
            Request::builder()
                .uri("/uploads/proof.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(proof.status(), StatusCode::OK);
}
