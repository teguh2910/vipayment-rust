use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vipayment_catalog::{config::CatalogConfig, migrate, routes, worker, AppState};
use vipayment_shared::db;
use vipayment_shared::internal::InternalAuth;
use vipayment_shared::provider::Provider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vipayment_catalog=info,tower_http=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = CatalogConfig::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    let internal = InternalAuth::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    db::ensure_database_dir(&cfg.database_url)?;
    std::fs::create_dir_all(&cfg.logo_dir)?;

    let pool = db::connect(&cfg.database_url).await?;
    migrate(&pool).await?;

    let provider = Arc::new(Provider::new(&cfg.provider));
    if !provider.signature_matches(cfg.provider.sign.as_deref()) {
        tracing::warn!(
            "PROVIDER_VIPRESELLER_SIGN tidak cocok dengan md5(API_ID + API_KEY); \
             memakai signature hasil hitung SDK"
        );
    }
    if !cfg.provider.enabled {
        tracing::warn!("provider VIP Reseller dinonaktifkan; katalog tidak akan diperbarui");
    }

    tracing::info!(
        provider = %cfg.provider.base_url,
        markup_percent = cfg.markup_percent,
        bind = %cfg.bind_addr,
        "starting vipayment-catalog"
    );

    let cfg = Arc::new(cfg);
    let state = AppState {
        pool: pool.clone(),
        cfg: cfg.clone(),
        provider: provider.clone(),
        internal,
    };

    worker::spawn(pool, provider, cfg.clone());

    let app = routes::router(state);
    let listener = TcpListener::bind(cfg.bind_addr).await?;

    tracing::info!("listening on http://{}", cfg.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
