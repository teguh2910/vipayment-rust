use super::{PaymentInstruction, PaymentProvider};
use crate::{
    config::{BankAccount, Config},
    error::AppError,
    format_idr,
};
use async_trait::async_trait;

pub struct ManualPayment {
    qris_image_url: Option<String>,
    bank_accounts: Vec<BankAccount>,
}

impl ManualPayment {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            qris_image_url: cfg.manual_qris_image_url.clone(),
            bank_accounts: cfg.manual_bank_accounts.clone(),
        }
    }
}

#[async_trait]
impl PaymentProvider for ManualPayment {
    fn name(&self) -> &'static str {
        "manual"
    }

    async fn create_payment(
        &self,
        invoice: &str,
        amount: i64,
        expires_at: &str,
    ) -> Result<PaymentInstruction, AppError> {
        let method = if self.qris_image_url.is_some() {
            "qris"
        } else if !self.bank_accounts.is_empty() {
            "bank_transfer"
        } else {
            "manual"
        };

        let mut instructions = vec![
            format!(
                "Bayar tepat Rp {} untuk invoice {invoice}.",
                format_idr(amount)
            ),
            "Upload bukti pembayaran di halaman checkout.".to_string(),
            "Pesanan diproses otomatis setelah pembayaran diverifikasi admin.".to_string(),
        ];

        if self.bank_accounts.is_empty() && self.qris_image_url.is_none() {
            instructions.push("Hubungi admin untuk instruksi pembayaran.".to_string());
        }

        Ok(PaymentInstruction {
            provider: "manual".to_string(),
            method: method.to_string(),
            amount,
            qris_image_url: self.qris_image_url.clone(),
            bank_accounts: self.bank_accounts.clone(),
            instructions,
            expires_at: expires_at.to_string(),
        })
    }
}
