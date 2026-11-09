//! Error types for VIPayment SDK.

use thiserror::Error;

/// Result alias for VIPayment SDK operations.
pub type Result<T> = std::result::Result<T, VipaymentError>;

/// Represents errors that can occur while interacting with the VIPayment API.
#[derive(Debug, Error)]
pub enum VipaymentError {
    /// An error occurred in the HTTP transport layer.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// An error occurred while serializing or deserializing JSON payloads.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// An API-level error returned by the VIPayment endpoint (result: false).
    #[error("VIPayment API error: {message}")]
    ApiError {
        /// Error message returned by VIPayment server.
        message: String,
    },

    /// Expected data field was missing or empty in the response.
    #[error("Response is missing expected data payload")]
    MissingData,

    /// Configuration or validation error.
    #[error("Configuration error: {0}")]
    Config(String),
}
