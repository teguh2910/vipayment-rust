use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vipayment_fulfillment::{config::Config, order_client::OrderClient, worker};
use vipayment_shared::provider::Provider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("vipayment_fulfillment=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = Config::from_env()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;

    if !cfg.enabled {
        tracing::warn!("fulfillment dinonaktifkan (FULFILLMENT_ENABLED=false); keluar");
        return Ok(());
    }

    let provider = Arc::new(Provider::new(&cfg.provider));
    if !provider.signature_matches(cfg.provider.sign.as_deref()) {
        tracing::warn!(
            "PROVIDER_VIPRESELLER_SIGN tidak cocok dengan md5(API_ID + API_KEY); \
             memakai signature hasil hitung SDK"
        );
    }

    tracing::info!(
        order_service = %cfg.order_service_url,
        provider = %cfg.provider.base_url,
        interval_secs = cfg.poll_interval.as_secs(),
        "starting vipayment-fulfillment"
    );

    let client = OrderClient::new(
        &cfg.order_service_url,
        &cfg.internal_token,
        std::time::Duration::from_secs(10),
    );

    let mut interval = tokio::time::interval(cfg.poll_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        interval.tick().await;

        if let Err(error) = worker::run_once(&client, &provider).await {
            tracing::warn!(%error, "fulfillment iteration failed");
        }
    }
}
