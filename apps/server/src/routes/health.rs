use crate::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};

pub async fn health_handler(State(state): State<AppState>) -> Json<Value> {
    match state.client.profile().await {
        Ok(resp) => {
            if resp.is_success() {
                let data = resp.data.unwrap_or_default();
                Json(json!({
                    "status": "ok",
                    "provider": {
                        "connected": true,
                        "username": data.username,
                        "balance": data.balance,
                    }
                }))
            } else {
                Json(json!({
                    "status": "degraded",
                    "provider": {
                        "connected": false,
                        "message": resp.message,
                    }
                }))
            }
        }
        Err(err) => Json(json!({
            "status": "degraded",
            "provider": {
                "connected": false,
                "error": err.to_string(),
            }
        })),
    }
}
