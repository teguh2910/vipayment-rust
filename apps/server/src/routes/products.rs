use crate::{error::AppError, AppState};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct PrepaidProductQuery {
    pub filter_type: Option<String>,
    pub filter_value: Option<String>,
    // Aliases
    pub r#type: Option<String>,
    pub brand: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GameProductQuery {
    pub filter_game: Option<String>,
    pub filter_status: Option<String>,
    // Aliases
    pub game: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnifiedProductQuery {
    pub kind: Option<String>,
    pub filter_type: Option<String>,
    pub filter_value: Option<String>,
    pub r#type: Option<String>,
    pub brand: Option<String>,
    pub filter_game: Option<String>,
    pub filter_status: Option<String>,
    pub game: Option<String>,
    pub status: Option<String>,
}

pub async fn get_prepaid_products(
    State(state): State<AppState>,
    Query(query): Query<PrepaidProductQuery>,
) -> Result<Json<Value>, AppError> {
    let filter_type = query.filter_type.or(query.r#type);
    let filter_value = query.filter_value.or(query.brand);

    let resp = state
        .client
        .service_prepaid(filter_type.as_deref(), filter_value.as_deref())
        .await?;

    let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(val))
}

pub async fn get_game_products(
    State(state): State<AppState>,
    Query(query): Query<GameProductQuery>,
) -> Result<Json<Value>, AppError> {
    let filter_game = query.filter_game.or(query.game);
    let filter_status = query.filter_status.or(query.status);

    let resp = state
        .client
        .service_game(filter_game.as_deref(), filter_status.as_deref())
        .await?;

    let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(val))
}

pub async fn get_products(
    State(state): State<AppState>,
    Query(query): Query<UnifiedProductQuery>,
) -> Result<Json<Value>, AppError> {
    match query.kind.as_deref() {
        Some("game") => {
            let filter_game = query.filter_game.or(query.game);
            let filter_status = query.filter_status.or(query.status);
            let resp = state
                .client
                .service_game(filter_game.as_deref(), filter_status.as_deref())
                .await?;
            let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(Json(val))
        }
        _ => {
            let filter_type = query.filter_type.or(query.r#type);
            let filter_value = query.filter_value.or(query.brand);
            let resp = state
                .client
                .service_prepaid(filter_type.as_deref(), filter_value.as_deref())
                .await?;
            let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
            Ok(Json(val))
        }
    }
}
