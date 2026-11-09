//! Integration tests for VIPayment Rust SDK using WireMock.

use vipayment::client::generate_signature;
use vipayment::{ApiResponse, ProfileData, VipaymentClient, VipaymentError};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_API_ID: &str = "test_id";
const TEST_API_KEY: &str = "test_key";

#[tokio::test]
async fn test_signature_generation() {
    let sign = generate_signature(TEST_API_ID, TEST_API_KEY);
    assert_eq!(sign, "76e228aaec26d1a075876fbb95dd3c10");
}

#[tokio::test]
async fn test_profile_success() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": true,
        "message": "Data didapatkan",
        "data": {
            "username": "teguh_vip",
            "name": "Teguh VIP",
            "balance": 250000,
            "level": "Reseller",
            "status": "active"
        }
    });

    Mock::given(method("POST"))
        .and(path("/profile"))
        .and(body_string_contains("key=test_key"))
        .and(body_string_contains(
            "sign=76e228aaec26d1a075876fbb95dd3c10",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client.profile().await.expect("Failed to get profile");
    assert!(res.is_success());
    assert_eq!(res.message, "Data didapatkan");

    let profile = res.data.expect("Missing profile data");
    assert_eq!(profile.username.as_deref(), Some("teguh_vip"));
    assert_eq!(profile.name.as_deref(), Some("Teguh VIP"));
    assert_eq!(profile.level.as_deref(), Some("Reseller"));
}

#[tokio::test]
async fn test_profile_api_failure() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": false,
        "message": "Invalid API key or signature"
    });

    Mock::given(method("POST"))
        .and(path("/profile"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res: ApiResponse<ProfileData> = client.profile().await.expect("Request failed");
    assert!(!res.is_success());
    assert_eq!(res.message, "Invalid API key or signature");

    // Test conversion with into_result()
    let err = res.into_result().unwrap_err();
    match err {
        VipaymentError::ApiError { message } => {
            assert_eq!(message, "Invalid API key or signature");
        }
        _ => panic!("Expected ApiError"),
    }
}

#[tokio::test]
async fn test_order_prepaid() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": true,
        "message": "Pesanan berhasil diproses",
        "data": {
            "trxid": "TRX12345",
            "code": "PLN5",
            "service": "PLN 5.000",
            "status": "waiting",
            "price": 5200
        }
    });

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=order"))
        .and(body_string_contains("service=PLN5"))
        .and(body_string_contains("data_no=1234567890"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client
        .order_prepaid("PLN5", "1234567890")
        .await
        .expect("order_prepaid failed");

    assert!(res.is_success());
    let data = res.into_result().expect("Data unwrap failed");
    assert_eq!(data.trxid.as_deref(), Some("TRX12345"));
    assert_eq!(data.service.as_deref(), Some("PLN 5.000"));
    assert_eq!(data.status.as_deref(), Some("waiting"));
}

#[tokio::test]
async fn test_status_order_prepaid() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": true,
        "message": "Status ditemukan",
        "data": [
            {
                "trxid": "TRX12345",
                "service": "PLN 5.000",
                "status": "success",
                "sn": "1234-5678-9012-3456",
                "price": 5200
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=status"))
        .and(body_string_contains("trxid=TRX12345"))
        .and(body_string_contains("limit=5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client
        .status_order_prepaid("TRX12345", Some(5))
        .await
        .expect("status_order_prepaid failed");

    assert!(res.is_success());
    let items = res.into_result().expect("Data unwrap failed");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status.as_deref(), Some("success"));
    assert_eq!(items[0].sn.as_deref(), Some("1234-5678-9012-3456"));
}

#[tokio::test]
async fn test_service_prepaid() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": true,
        "message": "Layanan ditemukan",
        "data": [
            {
                "code": "PULSA5",
                "name": "Pulsa Reguler 5.000",
                "price": 5500,
                "status": "available",
                "brand": "Telkomsel",
                "type": "pulsa-reguler"
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=services"))
        .and(body_string_contains("filter_type=type"))
        .and(body_string_contains("filter_value=pulsa-reguler"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client
        .service_prepaid(Some("type"), Some("pulsa-reguler"))
        .await
        .expect("service_prepaid failed");

    assert!(res.is_success());
    let items = res.into_result().expect("Data unwrap failed");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].code.as_deref(), Some("PULSA5"));
    assert_eq!(items[0].brand.as_deref(), Some("Telkomsel"));
    assert_eq!(items[0].service_type.as_deref(), Some("pulsa-reguler"));
}

#[tokio::test]
async fn test_order_game() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": true,
        "message": "Order game berhasil",
        "data": {
            "trxid": "GAME999",
            "service": "ML5",
            "data_no": "12345678",
            "data_zone": "1234",
            "status": "waiting",
            "price": 15000
        }
    });

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=order"))
        .and(body_string_contains("service=ML5"))
        .and(body_string_contains("data_no=12345678"))
        .and(body_string_contains("data_zone=1234"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client
        .order_game("ML5", "12345678", Some("1234"))
        .await
        .expect("order_game failed");

    assert!(res.is_success());
    let data = res.into_result().expect("Data unwrap failed");
    assert_eq!(data.trxid.as_deref(), Some("GAME999"));
    assert_eq!(data.service.as_deref(), Some("ML5"));
}

#[tokio::test]
async fn test_status_order_game() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": true,
        "message": "Status game ditemukan",
        "data": [
            {
                "trxid": "GAME999",
                "service": "ML5",
                "status": "success",
                "sn": "DIAMONDS-SENT-OK"
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=status"))
        .and(body_string_contains("trxid=GAME999"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client
        .status_order_game("GAME999", None)
        .await
        .expect("status_order_game failed");

    assert!(res.is_success());
    let items = res.into_result().expect("Data unwrap failed");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].sn.as_deref(), Some("DIAMONDS-SENT-OK"));
}

#[tokio::test]
async fn test_service_game() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "result": true,
        "message": "Layanan game ditemukan",
        "data": [
            {
                "code": "ML86",
                "game": "Mobile Legends",
                "name": "86 Diamonds",
                "price": 20000,
                "status": "available"
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=services"))
        .and(body_string_contains("filter_game=Mobile+Legends"))
        .and(body_string_contains("filter_status=available"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = VipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client
        .service_game(Some("Mobile Legends"), Some("available"))
        .await
        .expect("service_game failed");

    assert!(res.is_success());
    let items = res.into_result().expect("Data unwrap failed");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].game.as_deref(), Some("Mobile Legends"));
    assert_eq!(items[0].code.as_deref(), Some("ML86"));
}

#[cfg(feature = "blocking")]
#[test]
fn test_blocking_client_profile() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mock_server = rt.block_on(MockServer::start());

    let response_body = serde_json::json!({
        "result": true,
        "message": "Success",
        "data": {
            "username": "blocking_user",
            "balance": 100000
        }
    });

    rt.block_on(
        Mock::given(method("POST"))
            .and(path("/profile"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&mock_server),
    );

    let client = vipayment::blocking::BlockingVipaymentClient::builder(TEST_API_ID, TEST_API_KEY)
        .base_url(mock_server.uri())
        .build()
        .unwrap();

    let res = client.profile().unwrap();
    assert!(res.is_success());
    let data = res.into_result().unwrap();
    assert_eq!(data.username.as_deref(), Some("blocking_user"));
}
