use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vipayment_server::{config::Config, routes, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vipayment_server=info,tower_http=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    let bind_addr = cfg.bind_addr.clone();
    tracing::info!(
        bind = %bind_addr,
        base_url = cfg.base_url.as_deref().unwrap_or("https://vip-reseller.co.id/api"),
        "starting vipayment-server (stateless order & product gateway)"
    );

    let state = AppState::new(cfg);
    let app = routes::router(state);

    let listener = TcpListener::bind(&bind_addr).await?;
    tracing::info!("listening on http://{}", bind_addr);
    axum::serve(listener, app).await?;

    Ok(())
}
