//! Internal API consumed by the fulfillment service. The order state machine
//! stays here: the fulfillment service only claims work and reports back.

use crate::{orders, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use vipayment_shared::status::OrderStatus;
use vipayment_shared::{now_rfc3339, rfc3339_after_seconds, AppError};

/// Lease length granted when an order is claimed. Matches the monolith so a
/// crashed fulfillment instance cannot double-submit within the window.
const SUBMIT_LEASE_SECONDS: i64 = 60;
const MAX_SUBMIT_ATTEMPTS: i64 = 5;

#[derive(Debug, Deserialize)]
pub struct SubmittedBody {
    pub provider_trxid: String,
    pub provider_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FailureBody {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct StatusBody {
    pub provider_status: String,
    pub sn: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PollQuery {
    pub limit: Option<i64>,
}

/// Atomically claims one paid order (respecting the submission lease) and marks
/// it `submitting`. Returns `204` when there is nothing to do.
pub async fn claim(State(state): State<AppState>) -> Result<(StatusCode, Json<Value>), AppError> {
    let now = now_rfc3339();
    let sql = format!(
        "SELECT {} FROM orders
         WHERE status = ? AND (submission_lease_until IS NULL OR submission_lease_until < ?)
         ORDER BY created_at LIMIT 1",
        orders::ORDER_COLUMNS
    );

    let Some(order) = sqlx::query_as::<_, orders::OrderRow>(&sql)
        .bind(OrderStatus::Paid.as_str())
        .bind(&now)
        .fetch_optional(&state.pool)
        .await?
    else {
        return Ok((StatusCode::NO_CONTENT, Json(Value::Null)));
    };

    let lease = rfc3339_after_seconds(SUBMIT_LEASE_SECONDS);
    let claimed = sqlx::query(
        "UPDATE orders SET status = ?, submission_lease_until = ?, updated_at = ?
         WHERE id = ? AND status = ?",
    )
    .bind(OrderStatus::Submitting.as_str())
    .bind(&lease)
    .bind(&now)
    .bind(order.id)
    .bind(OrderStatus::Paid.as_str())
    .execute(&state.pool)
    .await?;

    if claimed.rows_affected() != 1 {
        // Another fulfillment instance won the race.
        return Ok((StatusCode::NO_CONTENT, Json(Value::Null)));
    }

    Ok((StatusCode::OK, Json(claim_json(&order))))
}

pub async fn submitted(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
    Json(body): Json<SubmittedBody>,
) -> Result<Json<Value>, AppError> {
    let order = load_order(&state, &invoice).await?;
    let now = now_rfc3339();
    let mut tx = state.pool.begin().await?;

    sqlx::query(
        "UPDATE orders SET status = ?, provider_trxid = ?, provider_status = ?,
         submitted_at = ?, submission_lease_until = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(OrderStatus::Processing.as_str())
    .bind(&body.provider_trxid)
    .bind(&body.provider_status)
    .bind(&now)
    .bind(&now)
    .bind(order.id)
    .execute(&mut *tx)
    .await?;

    orders::insert_event(
        &mut *tx,
        order.id,
        Some(OrderStatus::Paid.as_str()),
        OrderStatus::Processing.as_str(),
        Some("order dikirim ke provider"),
    )
    .await?;

    tx.commit().await?;
    tracing::info!(invoice = %invoice, trxid = %body.provider_trxid, "order submitted to provider");

    Ok(Json(json!({ "ok": true })))
}

/// The provider rejected the order outright: it needs a refund.
pub async fn rejected(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
    Json(body): Json<FailureBody>,
) -> Result<Json<Value>, AppError> {
    let order = load_order(&state, &invoice).await?;
    let now = now_rfc3339();
    let mut tx = state.pool.begin().await?;

    sqlx::query(
        "UPDATE orders SET status = ?, failure_reason = ?,
         submission_lease_until = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(OrderStatus::RefundPending.as_str())
    .bind(&body.message)
    .bind(&now)
    .bind(order.id)
    .execute(&mut *tx)
    .await?;

    orders::insert_event(
        &mut *tx,
        order.id,
        Some(OrderStatus::Submitting.as_str()),
        OrderStatus::RefundPending.as_str(),
        Some(&format!("ditolak provider: {}", body.message)),
    )
    .await?;

    tx.commit().await?;
    tracing::warn!(invoice = %invoice, message = %body.message, "provider rejected order");

    Ok(Json(json!({ "ok": true })))
}

/// A transport/provider failure: retry with backoff, or flag for admin attention
/// once the attempt budget is exhausted.
pub async fn failed(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
    Json(body): Json<FailureBody>,
) -> Result<Json<Value>, AppError> {
    let order = load_order(&state, &invoice).await?;
    let attempts = order.submit_attempts + 1;
    let now = now_rfc3339();

    if attempts >= MAX_SUBMIT_ATTEMPTS {
        sqlx::query(
            "UPDATE orders SET status = ?, submit_attempts = ?, failure_reason = ?,
             submission_lease_until = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(OrderStatus::NeedsAttention.as_str())
        .bind(attempts)
        .bind(&body.message)
        .bind(&now)
        .bind(order.id)
        .execute(&state.pool)
        .await?;

        orders::insert_event(
            &state.pool,
            order.id,
            Some(OrderStatus::Submitting.as_str()),
            OrderStatus::NeedsAttention.as_str(),
            Some("gagal mengirim order ke provider, perlu tindakan admin"),
        )
        .await?;

        tracing::error!(invoice = %invoice, message = %body.message, "order needs attention");
        return Ok(Json(json!({ "ok": true, "status": "needs_attention" })));
    }

    let backoff_seconds = (10i64 * (1i64 << attempts)).min(300);
    let lease = rfc3339_after_seconds(backoff_seconds);

    sqlx::query(
        "UPDATE orders SET status = ?, submit_attempts = ?, failure_reason = ?,
         submission_lease_until = ?, updated_at = ? WHERE id = ?",
    )
    .bind(OrderStatus::Paid.as_str())
    .bind(attempts)
    .bind(&body.message)
    .bind(&lease)
    .bind(&now)
    .bind(order.id)
    .execute(&state.pool)
    .await?;

    tracing::warn!(
        invoice = %invoice,
        attempt = attempts,
        message = %body.message,
        "provider submit failed, will retry"
    );

    Ok(Json(
        json!({ "ok": true, "status": "paid", "attempts": attempts }),
    ))
}

/// Orders the fulfillment service should poll at the provider.
pub async fn pollable(
    State(state): State<AppState>,
    Query(query): Query<PollQuery>,
) -> Result<Json<Value>, AppError> {
    let limit = query.limit.unwrap_or(5).clamp(1, 50);
    let sql = format!(
        "SELECT {} FROM orders
         WHERE status = ? AND provider_trxid IS NOT NULL
         ORDER BY updated_at LIMIT ?",
        orders::ORDER_COLUMNS
    );

    let rows = sqlx::query_as::<_, orders::OrderRow>(&sql)
        .bind(OrderStatus::Processing.as_str())
        .bind(limit)
        .fetch_all(&state.pool)
        .await?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|order| {
            json!({
                "invoice": order.invoice,
                "kind": order.kind,
                "provider_trxid": order.provider_trxid,
                "provider_status": order.provider_status,
            })
        })
        .collect();

    Ok(Json(json!({ "orders": items })))
}

/// Applies a provider status report. `mapped` is the order status the
/// fulfillment service derived from the raw provider status.
pub async fn apply_status(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
    Json(body): Json<StatusBody>,
) -> Result<Json<Value>, AppError> {
    let order = load_order(&state, &invoice).await?;
    let mapped = vipayment_shared::provider::map_provider_status(&body.provider_status);
    let now = now_rfc3339();

    match mapped {
        OrderStatus::Success => {
            let mut tx = state.pool.begin().await?;

            sqlx::query(
                "UPDATE orders SET status = ?, provider_status = ?,
                 provider_sn = COALESCE(?, provider_sn), completed_at = ?,
                 submission_lease_until = NULL, updated_at = ? WHERE id = ?",
            )
            .bind(OrderStatus::Success.as_str())
            .bind(&body.provider_status)
            .bind(&body.sn)
            .bind(&now)
            .bind(&now)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;

            orders::insert_event(
                &mut *tx,
                order.id,
                Some(OrderStatus::Processing.as_str()),
                OrderStatus::Success.as_str(),
                Some("order sukses diproses provider"),
            )
            .await?;

            tx.commit().await?;
            tracing::info!(invoice = %invoice, "order completed");
        }
        OrderStatus::Failed => {
            let mut tx = state.pool.begin().await?;

            sqlx::query(
                "UPDATE orders SET status = ?, provider_status = ?, failure_reason = ?,
                 submission_lease_until = NULL, updated_at = ? WHERE id = ?",
            )
            .bind(OrderStatus::RefundPending.as_str())
            .bind(&body.provider_status)
            .bind(&body.note)
            .bind(&now)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;

            orders::insert_event(
                &mut *tx,
                order.id,
                Some(OrderStatus::Processing.as_str()),
                OrderStatus::RefundPending.as_str(),
                Some("provider melaporkan kegagalan, perlu refund"),
            )
            .await?;

            tx.commit().await?;
            tracing::warn!(invoice = %invoice, status = %body.provider_status, "order failed");
        }
        _ => {
            let changed = order.provider_status.as_deref() != Some(body.provider_status.as_str())
                || body.sn.is_some();

            if changed {
                sqlx::query(
                    "UPDATE orders SET provider_status = ?,
                     provider_sn = COALESCE(?, provider_sn), updated_at = ? WHERE id = ?",
                )
                .bind(&body.provider_status)
                .bind(&body.sn)
                .bind(&now)
                .bind(order.id)
                .execute(&state.pool)
                .await?;
            }
        }
    }

    Ok(Json(json!({ "ok": true, "status": mapped.as_str() })))
}

async fn load_order(state: &AppState, invoice: &str) -> Result<orders::OrderRow, AppError> {
    orders::find_order(&state.pool, invoice)
        .await?
        .ok_or_else(|| AppError::NotFound("invoice tidak ditemukan".into()))
}

fn claim_json(order: &orders::OrderRow) -> Value {
    json!({
        "invoice": order.invoice,
        "kind": order.kind,
        "product_code": order.product_code,
        "user_id": order.user_id,
        "zone": order.zone,
        "submit_attempts": order.submit_attempts,
    })
}
