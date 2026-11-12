use crate::{catalog, error::AppError, orders, AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub status: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    pub reason: String,
}

pub async fn payments(
    State(state): State<AppState>,
    Query(query): Query<StatusQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::list_payments(&state.pool, query.status.as_deref(), query.q.as_deref()).await?,
    ))
}

pub async fn orders(
    State(state): State<AppState>,
    Query(query): Query<StatusQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::list_orders(&state.pool, query.status.as_deref(), query.q.as_deref()).await?,
    ))
}

pub async fn approve_payment(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::approve_payment(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?,
    ))
}

pub async fn reject_payment(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
    Json(body): Json<RejectRequest>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::reject_payment(
            &state.pool,
            &state.cfg,
            state.payments.as_ref(),
            &invoice,
            &body.reason,
        )
        .await?,
    ))
}

pub async fn retry_order(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::retry_order(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?,
    ))
}

pub async fn mark_refunded(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::mark_refunded(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?,
    ))
}

pub async fn sync_products(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let report = catalog::sync(&state.pool, &state.provider, &state.cfg).await?;
    Ok(Json(serde_json::to_value(report).unwrap_or(Value::Null)))
}

pub async fn profile(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let profile = state.provider.profile().await?;

    Ok(Json(json!({
        "profile": serde_json::to_value(profile).unwrap_or(Value::Null),
        "signature_ok": state
            .provider
            .signature_matches(state.cfg.provider_sign.as_deref()),
    })))
}
