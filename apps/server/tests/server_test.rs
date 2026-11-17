use reqwest::StatusCode;
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use vipayment_server::{config::Config, routes, AppState};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn spawn_test_server(mock_base_url: &str) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let cfg = Config {
        bind_addr: "127.0.0.1:0".to_string(),
        api_id: "test_id".to_string(),
        api_key: "test_key".to_string(),
        base_url: Some(mock_base_url.to_string()),
        cors_origin: "*".to_string(),
    };

    let state = AppState::new(cfg);
    let app = routes::router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, handle)
}

#[tokio::test]
async fn test_health_check() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/profile"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Success",
            "data": {
                "username": "reseller_pro",
                "balance": 750000
            }
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{addr}/api/v1/health"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["provider"]["connected"], true);
    assert_eq!(body["provider"]["username"], "reseller_pro");
    assert_eq!(body["provider"]["balance"], 750000);
}

#[tokio::test]
async fn test_get_prepaid_products() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=services"))
        .and(body_string_contains("filter_type=type"))
        .and(body_string_contains("filter_value=pulsa-reguler"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Sukses",
            "data": [
                {
                    "code": "S5",
                    "name": "Telkomsel 5.000",
                    "price": 5300,
                    "status": "available"
                }
            ]
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "http://{addr}/api/v1/products/prepaid?filter_type=type&filter_value=pulsa-reguler"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], true);
    assert_eq!(body["data"][0]["code"], "S5");
    assert_eq!(body["data"][0]["price"], 5300);
}

#[tokio::test]
async fn test_get_game_products() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=services"))
        .and(body_string_contains("filter_game=Mobile+Legends"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Sukses",
            "data": [
                {
                    "code": "ML86",
                    "game": "Mobile Legends",
                    "name": "86 Diamonds",
                    "price": 19500,
                    "status": "available"
                }
            ]
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "http://{addr}/api/v1/products/game?filter_game=Mobile%20Legends"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], true);
    assert_eq!(body["data"][0]["code"], "ML86");
    assert_eq!(body["data"][0]["price"], 19500);
}

#[tokio::test]
async fn test_order_prepaid_success() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=order"))
        .and(body_string_contains("service=PLN50"))
        .and(body_string_contains("data_no=1234567890"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Pesanan berhasil",
            "data": {
                "trxid": "VIP-TRX-1001",
                "service": "PLN50",
                "data_no": "1234567890",
                "status": "waiting"
            }
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/v1/orders/prepaid"))
        .json(&json!({
            "service_code": "PLN50",
            "data_no": "1234567890"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], true);
    assert_eq!(body["data"]["trxid"], "VIP-TRX-1001");
    assert_eq!(body["data"]["status"], "waiting");
}

#[tokio::test]
async fn test_order_game_success() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=order"))
        .and(body_string_contains("service=ML86"))
        .and(body_string_contains("data_no=12345678"))
        .and(body_string_contains("data_zone=1234"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Pesanan game berhasil",
            "data": {
                "trxid": "VIP-GAME-2002",
                "service": "ML86",
                "data_no": "12345678",
                "data_zone": "1234",
                "status": "waiting"
            }
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/v1/orders/game"))
        .json(&json!({
            "service_code": "ML86",
            "data_no": "12345678",
            "data_zone": "1234"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], true);
    assert_eq!(body["data"]["trxid"], "VIP-GAME-2002");
}

#[tokio::test]
async fn test_order_status_check() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=status"))
        .and(body_string_contains("trxid=VIP-TRX-1001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Detail transaksi",
            "data": [
                {
                    "trxid": "VIP-TRX-1001",
                    "service": "PLN50",
                    "status": "success",
                    "sn": "1234-5678-9012-3456"
                }
            ]
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{addr}/api/v1/orders/VIP-TRX-1001"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], true);
    assert_eq!(body["data"][0]["trxid"], "VIP-TRX-1001");
    assert_eq!(body["data"][0]["status"], "success");
    assert_eq!(body["data"][0]["sn"], "1234-5678-9012-3456");
}

#[tokio::test]
async fn test_get_unified_products() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=services"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Sukses",
            "data": [
                {
                    "code": "PLN20",
                    "name": "PLN 20.000",
                    "price": 20500,
                    "status": "available"
                }
            ]
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{addr}/api/v1/products?kind=prepaid"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], true);
    assert_eq!(body["data"][0]["code"], "PLN20");
}

#[tokio::test]
async fn test_create_unified_order() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=order"))
        .and(body_string_contains("service=FF70"))
        .and(body_string_contains("data_no=999888"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "Pesanan FF berhasil",
            "data": {
                "trxid": "VIP-GAME-3003",
                "service": "FF70",
                "data_no": "999888",
                "status": "waiting"
            }
        })))
        .mount(&mock)
        .await;

    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&json!({
            "kind": "game",
            "service_code": "FF70",
            "data_no": "999888"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], true);
    assert_eq!(body["data"]["trxid"], "VIP-GAME-3003");
}

#[tokio::test]
async fn test_order_missing_fields_validation() {
    let mock = MockServer::start().await;
    let (addr, _server) = spawn_test_server(&mock.uri()).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{addr}/api/v1/orders/prepaid"))
        .json(&json!({
            "service_code": "PLN50",
            "data_no": ""
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], false);
}
