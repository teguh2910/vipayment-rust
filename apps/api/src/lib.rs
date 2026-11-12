pub mod catalog;
pub mod config;
pub mod db;
pub mod error;
pub mod orders;
pub mod payments;
pub mod provider;
pub mod routes;
pub mod status;
pub mod uploads;
pub mod util;
pub mod worker;

use payments::PaymentProvider;
use provider::Provider;
use sqlx::SqlitePool;
use std::sync::Arc;

pub use util::{
    constant_time_eq, format_idr, mask_contact, now_rfc3339, rfc3339_after_minutes,
    rfc3339_after_seconds,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<config::Config>,
    pub provider: Arc<Provider>,
    pub payments: Arc<dyn PaymentProvider>,
}
