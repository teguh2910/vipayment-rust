use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use vipayment_order::{
    catalog_client::CatalogClient,
    config::{Config, PaymentProviderKind},
    migrate, orders, payments, AppState,
};
use vipayment_shared::db;
use vipayment_shared::internal::InternalAuth;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn test_pool() -> SqlitePool {
    let pool = db::connect("sqlite::memory:").await.unwrap();
    migrate(&pool).await.unwrap();
    pool
}

/// Catalog service double: serves the resolved product the order service needs
/// at checkout. Individual tests override the response for the unavailable or
/// hidden cases.
async fn mount_catalog(server: &MockServer, game_slug: &str, product_code: &str) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/internal/catalog/resolve/{game_slug}/{product_code}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "game": {
                "slug": game_slug,
                "name": "Mobile Legends",
                "icon_url": null,
                "requires_zone": true,
                "user_id_label": "User ID",
                "zone_label": "Zone ID",
                "kind": "game",
                "category": null
            },
            "product": {
                "code": product_code,
                "name": "86 Diamonds",
                "base_price": 19000,
                "sell_price": 19950
            }
        })))
        .mount(server)
        .await;
}

fn test_config(catalog_url: String) -> Config {
    Config {
        payment_provider: PaymentProviderKind::Simulation,
        simulation_auto_pay_seconds: 0,
        catalog_service_url: catalog_url,
        internal_token: "test-internal-token".to_string(),
        admin_token: "test-admin-token".to_string(),
        ..Config::default()
    }
}

fn catalog_client(cfg: &Config) -> CatalogClient {
    CatalogClient::new(
        &cfg.catalog_service_url,
        &cfg.internal_token,
        cfg.catalog_timeout(),
    )
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
async fn creates_an_order_with_prices_resolved_from_the_catalog() {
    let server = MockServer::start().await;
    mount_catalog(&server, "mobile-legends", "ML86").await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let created = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap();

    assert_eq!(created["status"], "pending_payment");
    assert_eq!(created["amount"], 19950);
    assert_eq!(created["product"]["code"], "ML86");
    assert_eq!(created["game"]["name"], "Mobile Legends");
    assert_eq!(created["game"]["requires_zone"], true);
    assert_eq!(created["account"]["user_id_label"], "User ID");
}

#[tokio::test]
async fn duplicate_idempotency_key_returns_the_same_invoice() {
    let server = MockServer::start().await;
    mount_catalog(&server, "mobile-legends", "ML86").await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let first = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap();
    let second = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap();

    assert_eq!(first["invoice"], second["invoice"]);
}

#[tokio::test]
async fn rejects_orders_for_unavailable_products() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/catalog/resolve/mobile-legends/ML86"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "error": "conflict",
            "message": "produk sedang tidak tersedia"
        })))
        .mount(&server)
        .await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let error = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, vipayment_shared::AppError::Conflict(_)));
}

#[tokio::test]
async fn requires_zone_for_games_that_need_it() {
    let server = MockServer::start().await;
    mount_catalog(&server, "mobile-legends", "ML86").await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let mut request = new_order_request();
    request.zone = None;

    let error = orders::create_order(&pool, &cfg, payments.as_ref(), &catalog, request)
        .await
        .unwrap_err();

    assert!(matches!(error, vipayment_shared::AppError::BadRequest(_)));
}

#[tokio::test]
async fn rejects_orders_the_catalog_hides_from_the_storefront() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/catalog/resolve/mobile-legends/ML86"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": "not_found",
            "message": "game tidak ditemukan"
        })))
        .mount(&server)
        .await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let error = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, vipayment_shared::AppError::NotFound(_)));
}

#[tokio::test]
async fn admin_approval_moves_the_order_to_paid() {
    let server = MockServer::start().await;
    mount_catalog(&server, "mobile-legends", "ML86").await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let created = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap();
    let invoice = created["invoice"].as_str().unwrap().to_string();

    orders::approve_payment(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();
    // Approving twice must be idempotent.
    let approved = orders::approve_payment(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();

    assert_eq!(approved["status"], "paid");
    assert_eq!(approved["payment"]["status"], "paid");
}

#[tokio::test]
async fn lookup_orders_by_contact_accepts_phone_variants() {
    let server = MockServer::start().await;
    mount_catalog(&server, "mobile-legends", "ML86").await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let created = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
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

    let unknown = orders::lookup_orders_by_contact(&pool, "089999999999")
        .await
        .unwrap();
    assert!(unknown["orders"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn proof_upload_is_only_allowed_while_payment_is_pending() {
    let server = MockServer::start().await;
    mount_catalog(&server, "mobile-legends", "ML86").await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let created = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap();
    let invoice = created["invoice"].as_str().unwrap().to_string();

    let order = orders::ensure_can_upload(&pool, &invoice).await.unwrap();
    orders::attach_proof(&pool, order.id, "proof.png")
        .await
        .unwrap();
    let detail = orders::order_detail(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();
    assert_eq!(detail["payment"]["status"], "awaiting_review");

    // A paid order can no longer accept a proof.
    orders::approve_payment(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();
    assert!(orders::ensure_can_upload(&pool, &invoice).await.is_err());
}

fn test_state(pool: SqlitePool, cfg: Config) -> AppState {
    let catalog = catalog_client(&cfg);
    let payments = payments::build(&cfg);
    let internal = InternalAuth::from_parts("test-internal-token".to_string());
    AppState {
        pool,
        cfg: Arc::new(cfg),
        payments,
        catalog,
        internal,
    }
}

#[tokio::test]
async fn internal_claim_respects_the_submission_lease() {
    let server = MockServer::start().await;
    mount_catalog(&server, "mobile-legends", "ML86").await;

    let pool = test_pool().await;
    let cfg = test_config(server.uri());
    let payments = payments::build(&cfg);
    let catalog = catalog_client(&cfg);

    let created = orders::create_order(
        &pool,
        &cfg,
        payments.as_ref(),
        &catalog,
        new_order_request(),
    )
    .await
    .unwrap();
    let invoice = created["invoice"].as_str().unwrap().to_string();

    let state = test_state(pool.clone(), cfg.clone());

    // Nothing to claim before payment.
    let (status, _) = vipayment_order::internal::claim(axum::extract::State(state.clone()))
        .await
        .unwrap();
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    orders::approve_payment(&pool, &cfg, payments.as_ref(), &invoice)
        .await
        .unwrap();

    let (status, body) = vipayment_order::internal::claim(axum::extract::State(state.clone()))
        .await
        .unwrap();
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["invoice"], invoice);

    // The lease is held, so a second instance must not claim the same order.
    let (status, _) = vipayment_order::internal::claim(axum::extract::State(state))
        .await
        .unwrap();
    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);

    let order = orders::find_order(&pool, &invoice).await.unwrap().unwrap();
    assert_eq!(order.status, "submitting");
}
