use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    PendingPayment,
    Paid,
    Submitting,
    Processing,
    Success,
    Failed,
    RefundPending,
    Refunded,
    Expired,
    NeedsAttention,
}

impl OrderStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PendingPayment => "pending_payment",
            Self::Paid => "paid",
            Self::Submitting => "submitting",
            Self::Processing => "processing",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::RefundPending => "refund_pending",
            Self::Refunded => "refunded",
            Self::Expired => "expired",
            Self::NeedsAttention => "needs_attention",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "pending_payment" => Self::PendingPayment,
            "paid" => Self::Paid,
            "submitting" => Self::Submitting,
            "processing" => Self::Processing,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "refund_pending" => Self::RefundPending,
            "refunded" => Self::Refunded,
            "expired" => Self::Expired,
            "needs_attention" => Self::NeedsAttention,
            _ => return None,
        })
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    AwaitingReview,
    Paid,
    Rejected,
    Expired,
}

impl PaymentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::AwaitingReview => "awaiting_review",
            Self::Paid => "paid",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "pending" => Self::Pending,
            "awaiting_review" => Self::AwaitingReview,
            "paid" => Self::Paid,
            "rejected" => Self::Rejected,
            "expired" => Self::Expired,
            _ => return None,
        })
    }
}
