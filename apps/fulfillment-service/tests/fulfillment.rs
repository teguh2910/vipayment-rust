//! End-to-end coverage for the split: a fake order service implements the
//! internal contract (including the submission lease) and the real provider
//! double is mocked. These mirror the original monolith `order_flow` tests that
//! guarded "submit exactly once".

use serde_json::json;
use vipayment_fulfillment::{order_client::OrderClient, worker};
use vipayment_shared::config::ProviderConfig;
use vipayment_shared::provider::Provider;
use wiremock::{
    matchers::{body_string_contains, method, path, path_regex},
    Mock, MockServer, ResponseTemplate,
};

async fn mount_order_service(server: &MockServer) {
    // The single paid order can only be claimed once; afterwards the queue is
    // empty. This is what proves the lease prevents a double submission.
    Mock::given(method("POST"))
        .and(path("/internal/orders/claim"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "invoice": "INV-1",
            "kind": "game",
            "product_code": "ML86",
            "user_id": "12345678",
            "zone": "1234"
        })))
        .up_to_n_times(1)
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/internal/orders/claim"))
        .respond_with(ResponseTemplate::new(204))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/internal/orders/pollable"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "orders": [{
                "invoice": "INV-1",
                "kind": "game",
                "provider_trxid": "TRX1",
                "provider_status": "waiting"
            }]
        })))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/internal/orders/INV-1/submitted"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path_regex(r"^/internal/orders/INV-1/status$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(server)
        .await;
}

fn provider_config(base_url: String) -> ProviderConfig {
    ProviderConfig {
        enabled: true,
        api_id: "test_id".to_string(),
        api_key: "test_key".to_string(),
        base_url,
        ..ProviderConfig::default()
    }
}

#[tokio::test]
async fn paid_order_is_submitted_once_and_status_is_reported() {
    let order_server = MockServer::start().await;
    let provider_server = MockServer::start().await;

    mount_order_service(&order_server).await;

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "order dibuat",
            "data": { "trxid": "TRX1", "status": "waiting", "price": 19000 }
        })))
        .mount(&provider_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "ok",
            "data": [{
                "trxid": "TRX1",
                "status": "success",
                "sn": "SN-123",
                "price": 19000
            }]
        })))
        .mount(&provider_server)
        .await;

    let provider = Provider::new(&provider_config(provider_server.uri()));
    let client = OrderClient::new(
        &order_server.uri(),
        "test-internal-token",
        std::time::Duration::from_secs(5),
    );

    worker::run_once(&client, &provider).await.unwrap();
    // A second iteration must not submit again: the fake order service returns
    // 204 once the single paid order has been claimed.
    worker::run_once(&client, &provider).await.unwrap();

    let requests = provider_server.received_requests().await.unwrap();
    let order_calls = requests
        .iter()
        .filter(|request| String::from_utf8_lossy(&request.body).contains("type=order"))
        .count();
    assert_eq!(order_calls, 1, "provider must be called exactly once");

    let submitted = order_server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.url.path().ends_with("/submitted"))
        .count();
    assert_eq!(submitted, 1, "order must be marked submitted exactly once");
}

#[tokio::test]
async fn prepaid_order_uses_the_prepaid_endpoint_without_zone() {
    let order_server = MockServer::start().await;
    let provider_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/internal/orders/claim"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "invoice": "INV-2",
            "kind": "prepaid",
            "product_code": "PLN20",
            "user_id": "51234567890",
            "zone": "999"
        })))
        .mount(&order_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/internal/orders/pollable"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "orders": [] })))
        .mount(&order_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/internal/orders/INV-2/submitted"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(&order_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "order dibuat",
            "data": { "trxid": "PP1", "status": "waiting", "price": 20000 }
        })))
        .mount(&provider_server)
        .await;

    let provider = Provider::new(&provider_config(provider_server.uri()));
    let client = OrderClient::new(
        &order_server.uri(),
        "test-internal-token",
        std::time::Duration::from_secs(5),
    );

    worker::run_once(&client, &provider).await.unwrap();

    let requests = provider_server.received_requests().await.unwrap();
    let bodies: Vec<String> = requests
        .iter()
        .filter(|request| String::from_utf8_lossy(&request.body).contains("type=order"))
        .map(|request| String::from_utf8_lossy(&request.body).to_string())
        .collect();

    assert_eq!(bodies.len(), 1);
    assert!(bodies[0].contains("service=PLN20"));
    assert!(bodies[0].contains("data_no=51234567890"));
    assert!(
        !bodies[0].contains("data_zone"),
        "prepaid orders must not send a zone"
    );
}

#[tokio::test]
async fn provider_rejection_is_reported_as_a_refund_case() {
    let order_server = MockServer::start().await;
    let provider_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/internal/orders/claim"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "invoice": "INV-3",
            "kind": "game",
            "product_code": "ML86",
            "user_id": "12345678",
            "zone": "1234"
        })))
        .mount(&order_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/internal/orders/pollable"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "orders": [] })))
        .mount(&order_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/internal/orders/INV-3/rejected"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
        .mount(&order_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": false,
            "message": "produk sedang gangguan",
            "data": null
        })))
        .mount(&provider_server)
        .await;

    let provider = Provider::new(&provider_config(provider_server.uri()));
    let client = OrderClient::new(
        &order_server.uri(),
        "test-internal-token",
        std::time::Duration::from_secs(5),
    );

    worker::run_once(&client, &provider).await.unwrap();

    let rejected = order_server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.url.path().ends_with("/rejected"))
        .count();
    assert_eq!(
        rejected, 1,
        "a provider rejection must be reported as a refund case"
    );
}
