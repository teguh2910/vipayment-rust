pub mod catalog;
pub mod config;
pub mod routes;
pub mod worker;

use sqlx::SqlitePool;
use std::sync::Arc;
use vipayment_shared::internal::InternalAuth;
use vipayment_shared::provider::Provider;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<config::CatalogConfig>,
    pub provider: Arc<Provider>,
    pub internal: InternalAuth,
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
