use crate::{catalog, config::CatalogConfig};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use vipayment_shared::db;
use vipayment_shared::provider::Provider;

pub fn spawn(pool: SqlitePool, provider: Arc<Provider>, cfg: Arc<CatalogConfig>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            match catalog::sync(&pool, &provider, &cfg).await {
                Ok(report) => tracing::info!(
                    groups = report.groups,
                    products = report.products,
                    logos = report.logos,
                    errors = report.errors.len(),
                    "catalog sync finished"
                ),
                Err(error) => {
                    tracing::warn!(%error, "catalog sync failed");
                    db::set_state(&pool, "last_sync_error", &error.to_string()).await;
                }
            }
        }
    });
}
