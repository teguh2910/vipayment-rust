mod manual;
mod simulation;

use crate::{
    config::{BankAccount, Config, PaymentProviderKind},
    error::AppError,
};
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct PaymentInstruction {
    pub provider: String,
    pub method: String,
    pub amount: i64,
    pub qris_image_url: Option<String>,
    pub bank_accounts: Vec<BankAccount>,
    pub instructions: Vec<String>,
    pub expires_at: String,
}

#[async_trait]
pub trait PaymentProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn create_payment(
        &self,
        invoice: &str,
        amount: i64,
        expires_at: &str,
    ) -> Result<PaymentInstruction, AppError>;
}

pub fn build(cfg: &Config) -> Arc<dyn PaymentProvider> {
    match cfg.payment_provider {
        PaymentProviderKind::Manual => Arc::new(manual::ManualPayment::from_config(cfg)),
        PaymentProviderKind::Simulation => {
            Arc::new(simulation::SimulationPayment::from_config(cfg))
        }
    }
}
