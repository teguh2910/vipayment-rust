use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use vipayment_api::{
    config::{Config, PaymentProviderKind},
    db, orders, payments,
    provider::Provider,
    worker::order_runner,
};
use wiremock::{
    matchers::{body_string_contains, method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn test_pool() -> SqlitePool {
    let pool = db::connect("sqlite::memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    pool
}

async fn seed_catalog(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO games (slug, name, provider_name, icon_url, requires_zone,
         user_id_label, zone_label, sort_order, active)
         VALUES ('mobile-legends', 'Mobile Legends', 'Mobile Legends', NULL, 1,
         'User ID', 'Zone ID', 1, 1)",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO products (code, game_slug, name, base_price, sell_price,
         provider_status, available, updated_at)
         VALUES ('ML86', 'mobile-legends', '86 Diamonds', 19000, 19950,
         'available', 1, '2026-01-01T00:00:00.000Z')",
    )
    .execute(pool)
    .await
    .unwrap();
}

fn test_config(base_url: String) -> Config {
    Config {
        provider_enabled: true,
        provider_api_id: "test_id".to_string(),
        provider_api_key: "test_key".to_string(),
        provider_base_url: base_url,
        provider_sign: Some(vipayment::generate_signature("test_id", "test_key")),
        payment_provider: PaymentProviderKind::Simulation,
        simulation_auto_pay_seconds: 0,
        ..Config::default()
    }
}

fn new_order_request() -> orders::CreateOrderRequest {
    orders::CreateOrderRequest {
        game_slug: "mobile-legends".to_string(),
        product_code: "ML86".to_string(),
        user_id: "12345678".to_string(),
        zone: Some("1234".to_string()),
        contact: "081234567890".to_string(),
        idempotency_key: Some("order-key-1".to_string()),
    }
}

#[tokio::test]
async fn paid_order_is_submitted_once_and_completed() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/game-feature"))
        .and(body_string_contains("type=order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "order dibuat",
            "data": { "trxid": "TRX1", "status": "waiting", "price": 19000 }
        })))
        .mount(&server)
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
        .mount(&server)
        .await;

    let pool = test_pool().await;
    seed_catalog(&pool).await;

    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);

    let created = orders::create_order(&pool, &cfg, payments.as_ref(), new_order_request())
        .await
        .unwrap();
    let invoice = created["invoice"].as_str().unwrap().to_string();
    assert_eq!(created["status"], "pending_payment");
    assert_eq!(created["amount"], 19950);

    orders::approve_payment(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();
    // Approving twice must be idempotent.
    orders::approve_payment(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();

    let provider = Arc::new(Provider::new(&cfg));
    order_runner::run_once(&pool, &provider).await.unwrap();
    order_runner::run_once(&pool, &provider).await.unwrap();

    let order = orders::find_order(&pool, &invoice).await.unwrap().unwrap();
    assert_eq!(order.status, "success");
    assert_eq!(order.provider_trxid.as_deref(), Some("TRX1"));
    assert_eq!(order.provider_sn.as_deref(), Some("SN-123"));
    assert!(order.completed_at.is_some());

    let requests = server.received_requests().await.unwrap();
    let order_calls: Vec<String> = requests
        .iter()
        .filter(|request| String::from_utf8_lossy(&request.body).contains("type=order"))
        .map(|request| String::from_utf8_lossy(&request.body).to_string())
        .collect();

    assert_eq!(order_calls.len(), 1, "provider must be called exactly once");
    let body = &order_calls[0];
    assert!(body.contains("service=ML86"));
    assert!(body.contains("data_no=12345678"));
    assert!(body.contains("data_zone=1234"));
}

#[tokio::test]
async fn duplicate_idempotency_key_returns_the_same_invoice() {
    let pool = test_pool().await;
    seed_catalog(&pool).await;

    let cfg = test_config("http://127.0.0.1:1".to_string());
    let payments = payments::build(&cfg);

    let first = orders::create_order(&pool, &cfg, payments.as_ref(), new_order_request())
        .await
        .unwrap();
    let second = orders::create_order(&pool, &cfg, payments.as_ref(), new_order_request())
        .await
        .unwrap();

    assert_eq!(first["invoice"], second["invoice"]);
}

#[tokio::test]
async fn rejects_orders_for_unavailable_products() {
    let pool = test_pool().await;
    seed_catalog(&pool).await;
    sqlx::query("UPDATE products SET available = 0 WHERE code = 'ML86'")
        .execute(&pool)
        .await
        .unwrap();

    let cfg = test_config("http://127.0.0.1:1".to_string());
    let payments = payments::build(&cfg);

    let error = orders::create_order(&pool, &cfg, payments.as_ref(), new_order_request())
        .await
        .unwrap_err();

    assert!(matches!(error, vipayment_api::error::AppError::Conflict(_)));
}

#[tokio::test]
async fn requires_zone_for_games_that_need_it() {
    let pool = test_pool().await;
    seed_catalog(&pool).await;

    let cfg = test_config("http://127.0.0.1:1".to_string());
    let payments = payments::build(&cfg);

    let mut request = new_order_request();
    request.zone = None;

    let error = orders::create_order(&pool, &cfg, payments.as_ref(), request)
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        vipayment_api::error::AppError::BadRequest(_)
    ));
}

async fn seed_prepaid_catalog(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO games (slug, name, provider_name, icon_url, requires_zone,
         user_id_label, zone_label, sort_order, active, kind, category)
         VALUES ('pp-token-pln-pln', 'PLN Token PLN', 'token-pln|PLN', NULL, 0,
         'ID Pelanggan / No. Meter', 'Zone ID', 1010, 1, 'prepaid', 'token-pln')",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO products (game_slug, code, name, brand, base_price, sell_price,
         provider_status, available, updated_at)
         VALUES ('pp-token-pln-pln', 'PLN20', 'Token PLN 20.000', 'PLN', 20000, 21000,
         'available', 1, '2026-01-01T00:00:00.000Z')",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn prepaid_order_uses_prepaid_endpoint_without_zone() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "order dibuat",
            "data": { "trxid": "PP1", "status": "waiting", "price": 20000 }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/prepaid"))
        .and(body_string_contains("type=status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "result": true,
            "message": "ok",
            "data": [{
                "trxid": "PP1",
                "status": "success",
                "sn": "SN-PLN-1",
                "price": 20000
            }]
        })))
        .mount(&server)
        .await;

    let pool = test_pool().await;
    seed_prepaid_catalog(&pool).await;

    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);

    let created = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        orders::CreateOrderRequest {
            game_slug: "pp-token-pln-pln".to_string(),
            product_code: "PLN20".to_string(),
            user_id: "51234567890".to_string(),
            // A zone is ignored for prepaid products.
            zone: Some("999".to_string()),
            contact: "081234567890".to_string(),
            idempotency_key: Some("prepaid-key-1".to_string()),
        },
    )
    .await
    .unwrap();

    let invoice = created["invoice"].as_str().unwrap().to_string();
    assert_eq!(created["game"]["kind"], "prepaid");

    orders::approve_payment(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();

    let provider = Arc::new(Provider::new(&cfg));
    order_runner::run_once(&pool, &provider).await.unwrap();
    order_runner::run_once(&pool, &provider).await.unwrap();

    let order = orders::find_order(&pool, &invoice).await.unwrap().unwrap();
    assert_eq!(order.kind, "prepaid");
    assert_eq!(order.status, "success");
    assert_eq!(order.provider_trxid.as_deref(), Some("PP1"));
    assert_eq!(order.provider_sn.as_deref(), Some("SN-PLN-1"));

    let requests = server.received_requests().await.unwrap();
    let order_bodies: Vec<String> = requests
        .iter()
        .filter(|request| String::from_utf8_lossy(&request.body).contains("type=order"))
        .map(|request| String::from_utf8_lossy(&request.body).to_string())
        .collect();

    assert_eq!(order_bodies.len(), 1);
    assert!(order_bodies[0].contains("service=PLN20"));
    assert!(order_bodies[0].contains("data_no=51234567890"));
    assert!(!order_bodies[0].contains("data_zone"));
}

#[tokio::test]
async fn lookup_orders_by_contact_accepts_phone_variants_and_email() {
    let pool = test_pool().await;
    seed_catalog(&pool).await;

    let cfg = test_config("http://127.0.0.1:1".to_string());
    let payments = payments::build(&cfg);

    let created = orders::create_order(&pool, &cfg, payments.as_ref(), new_order_request())
        .await
        .unwrap();
    let invoice = created["invoice"].as_str().unwrap().to_string();

    // Stored as "081234567890"; look it up with international formatting.
    let by_phone = orders::lookup_orders_by_contact(&pool, "+62 812-3456-7890")
        .await
        .unwrap();
    let phone_orders = by_phone["orders"].as_array().unwrap();
    assert_eq!(phone_orders.len(), 1);
    assert_eq!(phone_orders[0]["invoice"], invoice);

    let by_exact_phone = orders::lookup_orders_by_contact(&pool, "081234567890")
        .await
        .unwrap();
    assert_eq!(by_exact_phone["orders"].as_array().unwrap().len(), 1);

    let email_order = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        orders::CreateOrderRequest {
            contact: "Teguh@Example.com".to_string(),
            idempotency_key: Some("email-key-1".to_string()),
            ..new_order_request()
        },
    )
    .await
    .unwrap();
    let email_invoice = email_order["invoice"].as_str().unwrap().to_string();

    let by_email = orders::lookup_orders_by_contact(&pool, "teguh@example.com")
        .await
        .unwrap();
    let email_orders = by_email["orders"].as_array().unwrap();
    assert_eq!(email_orders.len(), 1);
    assert_eq!(email_orders[0]["invoice"], email_invoice);

    let unknown = orders::lookup_orders_by_contact(&pool, "089999999999")
        .await
        .unwrap();
    assert!(unknown["orders"].as_array().unwrap().is_empty());
}
