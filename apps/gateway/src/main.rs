use axum::{extract::DefaultBodyLimit, routing::any, Router};
use std::sync::Arc;
use tower_http::{limit::RequestBodyLimitLayer, trace::TraceLayer};
use vipayment_gateway::{config::Config, cors::cors_layer, proxy::Gateway};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("vipayment_gateway=info,tower_http=info")
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let cfg = Config::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let bind_addr = cfg.bind_addr;

    tracing::info!(
        catalog = %cfg.catalog_service_url,
        order = %cfg.order_service_url,
        bind = %bind_addr,
        origins = ?cfg.web_origins,
        allow_any_origin = cfg.allows_any_origin(),
        "starting vipayment-gateway"
    );

    if cfg.allows_any_origin() {
        tracing::warn!(
            "WEB_ORIGIN=* allows any website to call this API; \\
             set an explicit origin list unless the API is meant to be public"
        );
    }

    let gateway = Gateway::new(Arc::new(cfg.clone()));

    let cors = cors_layer(&cfg);

    let app = Router::new()
        .route("/api/v1/health", axum::routing::get(Gateway::health))
        .route("/api/v1/{*path}", any(Gateway::proxy_api))
        .route("/api/v1/", any(Gateway::proxy_api))
        .route("/uploads/{*path}", any(Gateway::proxy_static))
        .route("/logos/{*path}", any(Gateway::proxy_static))
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(3 * 1024 * 1024))
        .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(gateway);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;

    tracing::info!("listening on http://{}", bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
