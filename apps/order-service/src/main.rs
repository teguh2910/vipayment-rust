use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vipayment_order::{
    catalog_client::CatalogClient, config::Config, maintenance, migrate, payments, router, AppState,
};
use vipayment_shared::db;
use vipayment_shared::internal::InternalAuth;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vipayment_order=info,tower_http=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let internal = InternalAuth::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    db::ensure_database_dir(&cfg.database_url)?;
    std::fs::create_dir_all(&cfg.upload_dir)?;

    let pool = db::connect(&cfg.database_url).await?;
    migrate(&pool).await?;

    tracing::info!(
        payment = cfg.payment_provider.as_str(),
        catalog = %cfg.catalog_service_url,
        bind = %cfg.bind_addr,
        "starting vipayment-order"
    );

    let cfg = Arc::new(cfg);
    let payments = payments::build(&cfg);
    let catalog = CatalogClient::new(
        &cfg.catalog_service_url,
        &cfg.internal_token,
        cfg.catalog_timeout(),
    );

    let state = AppState {
        pool: pool.clone(),
        cfg: cfg.clone(),
        payments,
        catalog,
        internal,
    };

    maintenance::spawn(pool, cfg.clone());

    let app = router::router(state);
    let listener = TcpListener::bind(cfg.bind_addr).await?;

    tracing::info!("listening on http://{}", cfg.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
