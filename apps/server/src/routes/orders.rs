use crate::{error::AppError, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct PrepaidOrderRequest {
    pub service_code: Option<String>,
    pub service: Option<String>,
    pub data_no: String,
}

#[derive(Debug, Deserialize)]
pub struct GameOrderRequest {
    pub service_code: Option<String>,
    pub service: Option<String>,
    pub data_no: String,
    pub data_zone: Option<String>,
    pub zone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnifiedOrderRequest {
    pub kind: Option<String>,
    pub service_code: Option<String>,
    pub service: Option<String>,
    pub data_no: String,
    pub data_zone: Option<String>,
    pub zone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OrderStatusQuery {
    pub kind: Option<String>,
    pub limit: Option<u32>,
}

pub async fn order_prepaid(
    State(state): State<AppState>,
    Json(payload): Json<PrepaidOrderRequest>,
) -> Result<Json<Value>, AppError> {
    let service_code = payload.service_code.or(payload.service).ok_or_else(|| {
        AppError::BadRequest("Field 'service_code' (or 'service') is required".into())
    })?;

    if payload.data_no.trim().is_empty() {
        return Err(AppError::BadRequest("Field 'data_no' is required".into()));
    }

    let resp = state
        .client
        .order_prepaid(&service_code, &payload.data_no)
        .await?;

    let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(val))
}

pub async fn order_game(
    State(state): State<AppState>,
    Json(payload): Json<GameOrderRequest>,
) -> Result<Json<Value>, AppError> {
    let service_code = payload.service_code.or(payload.service).ok_or_else(|| {
        AppError::BadRequest("Field 'service_code' (or 'service') is required".into())
    })?;

    if payload.data_no.trim().is_empty() {
        return Err(AppError::BadRequest("Field 'data_no' is required".into()));
    }

    let data_zone = payload.data_zone.or(payload.zone);

    let resp = state
        .client
        .order_game(&service_code, &payload.data_no, data_zone.as_deref())
        .await?;

    let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(val))
}

pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<UnifiedOrderRequest>,
) -> Result<Json<Value>, AppError> {
    let service_code = payload.service_code.or(payload.service).ok_or_else(|| {
        AppError::BadRequest("Field 'service_code' (or 'service') is required".into())
    })?;

    if payload.data_no.trim().is_empty() {
        return Err(AppError::BadRequest("Field 'data_no' is required".into()));
    }

    let is_game = payload
        .kind
        .as_deref()
        .map(|k| k.eq_ignore_ascii_case("game"))
        .unwrap_or(false);

    if is_game {
        let data_zone = payload.data_zone.or(payload.zone);
        let resp = state
            .client
            .order_game(&service_code, &payload.data_no, data_zone.as_deref())
            .await?;
        let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(Json(val))
    } else {
        let resp = state
            .client
            .order_prepaid(&service_code, &payload.data_no)
            .await?;
        let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(Json(val))
    }
}

pub async fn get_order_status(
    State(state): State<AppState>,
    Path(trxid): Path<String>,
    Query(query): Query<OrderStatusQuery>,
) -> Result<Json<Value>, AppError> {
    let is_game = query
        .kind
        .as_deref()
        .map(|k| k.eq_ignore_ascii_case("game"))
        .unwrap_or(false);

    if is_game {
        let resp = state.client.status_order_game(&trxid, query.limit).await?;
        let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(Json(val))
    } else {
        let resp = state
            .client
            .status_order_prepaid(&trxid, query.limit)
            .await?;
        let val = serde_json::to_value(resp).map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(Json(val))
    }
}
