use super::{PaymentInstruction, PaymentProvider};
use crate::{config::Config, error::AppError, format_idr};
use async_trait::async_trait;

pub struct SimulationPayment {
    auto_pay_seconds: i64,
}

impl SimulationPayment {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            auto_pay_seconds: cfg.simulation_auto_pay_seconds,
        }
    }
}

#[async_trait]
impl PaymentProvider for SimulationPayment {
    fn name(&self) -> &'static str {
        "simulation"
    }

    async fn create_payment(
        &self,
        invoice: &str,
        amount: i64,
        expires_at: &str,
    ) -> Result<PaymentInstruction, AppError> {
        Ok(PaymentInstruction {
            provider: "simulation".to_string(),
            method: "simulation".to_string(),
            amount,
            qris_image_url: None,
            bank_accounts: Vec::new(),
            instructions: vec![
                format!(
                    "Mode simulasi: Rp {} untuk invoice {invoice} akan lunas otomatis.",
                    format_idr(amount)
                ),
                format!(
                    "Pembayaran dikonfirmasi otomatis dalam {} detik, lalu order dikirim ke provider.",
                    self.auto_pay_seconds
                ),
                "Tidak ada uang sungguhan yang ditarik pada mode ini.".to_string(),
            ],
            expires_at: expires_at.to_string(),
        })
    }
}
