pub mod catalog_client;
pub mod config;
pub mod internal;
pub mod maintenance;
pub mod orders;
pub mod payments;
pub mod router;
pub mod routes;
pub mod uploads;

use catalog_client::CatalogClient;
use payments::PaymentProvider;
use sqlx::SqlitePool;
use std::sync::Arc;
use vipayment_shared::internal::InternalAuth;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<config::Config>,
    pub payments: Arc<dyn PaymentProvider>,
    pub catalog: CatalogClient,
    pub internal: InternalAuth,
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
