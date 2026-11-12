use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vipayment_api::{config::Config, db, payments, provider::Provider, routes, worker, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vipayment_api=info,tower_http=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    db::ensure_database_dir(&cfg.database_url)?;
    std::fs::create_dir_all(&cfg.upload_dir)?;

    let pool = db::connect(&cfg.database_url).await?;
    db::migrate(&pool).await?;

    let provider = Arc::new(Provider::new(&cfg));
    if !provider.signature_matches(cfg.provider_sign.as_deref()) {
        tracing::warn!(
            "PROVIDER_VIPRESELLER_SIGN tidak cocok dengan md5(API_ID + API_KEY); \
             memakai signature hasil hitung SDK"
        );
    }
    if !cfg.provider_enabled {
        tracing::warn!("provider VIP Reseller dinonaktifkan; order baru akan ditolak");
    }

    tracing::info!(
        provider = %cfg.provider_base_url,
        payment = cfg.payment_provider.as_str(),
        markup_percent = cfg.markup_percent,
        bind = %cfg.bind_addr,
        "starting vipayment-api"
    );

    let cfg = Arc::new(cfg);
    let payments = payments::build(&cfg);
    let state = AppState {
        pool: pool.clone(),
        cfg: cfg.clone(),
        provider: provider.clone(),
        payments,
    };

    worker::catalog_sync::spawn(pool.clone(), provider.clone(), cfg.clone());
    worker::order_runner::spawn(pool.clone(), provider.clone());
    worker::maintenance::spawn(pool.clone(), cfg.clone());

    let app = routes::router(state);
    let listener = TcpListener::bind(cfg.bind_addr).await?;

    tracing::info!("listening on http://{}", cfg.bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
