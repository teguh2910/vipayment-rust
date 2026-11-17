use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;
use vipayment::VipaymentError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<VipaymentError> for AppError {
    fn from(err: VipaymentError) -> Self {
        match err {
            VipaymentError::ApiError { message } => AppError::Provider(message),
            VipaymentError::Http(e) => {
                AppError::Provider(format!("Provider connection error: {e}"))
            }
            VipaymentError::Json(e) => AppError::Internal(format!("Serialization error: {e}")),
            VipaymentError::MissingData => {
                AppError::Provider("Provider returned empty data".to_string())
            }
            VipaymentError::Config(e) => AppError::Internal(format!("Configuration error: {e}")),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Provider(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(json!({
            "result": false,
            "message": message,
        }));

        (status, body).into_response()
    }
}
