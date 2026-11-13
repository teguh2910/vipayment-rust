use crate::{config::var, constant_time_eq};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub const INTERNAL_TOKEN_HEADER: &str = "x-internal-token";

/// Shared secret that service-to-service calls must present. This is separate
/// from `ADMIN_TOKEN`: it authenticates services, not operators.
#[derive(Clone)]
pub struct InternalAuth {
    token: String,
}

impl InternalAuth {
    pub fn from_env() -> Result<Self, String> {
        let token =
            var("INTERNAL_TOKEN").ok_or_else(|| "INTERNAL_TOKEN wajib diisi".to_string())?;
        if token.len() < 16 {
            return Err("INTERNAL_TOKEN minimal 16 karakter".to_string());
        }
        Ok(Self { token })
    }

    /// Builds an instance from an already-validated token (used by tests).
    pub fn from_parts(token: String) -> Self {
        Self { token }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn matches(&self, provided: &str) -> bool {
        constant_time_eq(provided, &self.token)
    }
}

pub fn extract_token(request: &Request) -> &str {
    request
        .headers()
        .get(INTERNAL_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

/// Middleware guarding `/internal/*` routes against the other services.
pub async fn require_internal(auth: InternalAuth, request: Request, next: Next) -> Response {
    if !auth.matches(extract_token(&request)) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized", "message": "unauthorized" })),
        )
            .into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_internal_tokens_in_constant_time() {
        let auth = InternalAuth {
            token: "a-very-secret-internal-token".to_string(),
        };
        assert!(auth.matches("a-very-secret-internal-token"));
        assert!(!auth.matches("a-very-secret-internal-toke"));
        assert!(!auth.matches(""));
    }
}
