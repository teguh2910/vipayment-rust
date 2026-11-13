use crate::{config::PaymentProviderKind, orders};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;

pub fn spawn(pool: SqlitePool, cfg: Arc<crate::config::Config>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            match orders::expire_pending(&pool).await {
                Ok(0) => {}
                Ok(count) => tracing::info!(count, "expired unpaid orders"),
                Err(error) => tracing::warn!(%error, "expiry sweep failed"),
            }

            if cfg.payment_provider == PaymentProviderKind::Simulation {
                match orders::simulation_autopay(&pool, cfg.simulation_auto_pay_seconds).await {
                    Ok(0) => {}
                    Ok(count) => tracing::info!(count, "simulated payments confirmed"),
                    Err(error) => tracing::warn!(%error, "simulation autopay failed"),
                }
            }
        }
    });
}
