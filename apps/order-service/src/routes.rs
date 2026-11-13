use crate::{orders, uploads};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use vipayment_shared::AppError;

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub status: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct LookupQuery {
    pub contact: String,
}

// ---- public ----

pub async fn create_order(
    State(state): State<crate::AppState>,
    Json(request): Json<orders::CreateOrderRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let view = orders::create_order(
        &state.pool,
        &state.cfg,
        state.payments.as_ref(),
        &state.catalog,
        request,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(view)))
}

pub async fn order_detail(
    State(state): State<crate::AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    let view =
        orders::order_detail(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?;

    Ok(Json(view))
}

pub async fn lookup_orders(
    State(state): State<crate::AppState>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::lookup_orders_by_contact(&state.pool, &query.contact).await?,
    ))
}

pub async fn upload_proof(
    State(state): State<crate::AppState>,
    Path(invoice): Path<String>,
    multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    let order = orders::ensure_can_upload(&state.pool, &invoice).await?;
    let filename = uploads::save_payment_proof(multipart, &state.cfg.upload_dir).await?;
    orders::attach_proof(&state.pool, order.id, &filename).await?;

    Ok(Json(json!({
        "ok": true,
        "invoice": invoice,
        "proof_url": format!("/uploads/{filename}"),
    })))
}

pub async fn health(State(state): State<crate::AppState>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "ok": true,
        "payment_provider": state.payments.name(),
    })))
}

// ---- admin ----

pub async fn admin_payments(
    State(state): State<crate::AppState>,
    Query(query): Query<StatusQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::list_payments(&state.pool, query.status.as_deref(), query.q.as_deref()).await?,
    ))
}

pub async fn admin_orders(
    State(state): State<crate::AppState>,
    Query(query): Query<StatusQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::list_orders(&state.pool, query.status.as_deref(), query.q.as_deref()).await?,
    ))
}

pub async fn approve_payment(
    State(state): State<crate::AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::approve_payment(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?,
    ))
}

pub async fn reject_payment(
    State(state): State<crate::AppState>,
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
    State(state): State<crate::AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::retry_order(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?,
    ))
}

pub async fn mark_refunded(
    State(state): State<crate::AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::mark_refunded(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?,
    ))
}
