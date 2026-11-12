use crate::{
    error::AppError,
    orders::{self, OrderRow, ORDER_COLUMNS},
    provider::{self, Provider},
    status::OrderStatus,
};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;

pub fn spawn(pool: SqlitePool, provider: Arc<Provider>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            if let Err(error) = run_once(&pool, &provider).await {
                tracing::error!(%error, "order runner iteration failed");
            }
        }
    });
}

pub async fn run_once(pool: &SqlitePool, provider: &Provider) -> Result<(), AppError> {
    submit_pending(pool, provider).await?;
    poll_processing(pool, provider).await?;
    Ok(())
}

async fn submit_pending(pool: &SqlitePool, provider: &Provider) -> Result<(), AppError> {
    let now = crate::now_rfc3339();
    let sql = format!(
        "SELECT {ORDER_COLUMNS} FROM orders
         WHERE status = ? AND (submission_lease_until IS NULL OR submission_lease_until < ?)
         ORDER BY created_at LIMIT 1"
    );

    let Some(order) = sqlx::query_as::<_, OrderRow>(&sql)
        .bind(OrderStatus::Paid.as_str())
        .bind(&now)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(());
    };

    let lease = crate::rfc3339_after_seconds(60);
    let claimed = sqlx::query(
        "UPDATE orders SET status = ?, submission_lease_until = ?, updated_at = ?
         WHERE id = ? AND status = ?",
    )
    .bind(OrderStatus::Submitting.as_str())
    .bind(&lease)
    .bind(&now)
    .bind(order.id)
    .bind(OrderStatus::Paid.as_str())
    .execute(pool)
    .await?;

    if claimed.rows_affected() != 1 {
        return Ok(());
    }

    let provider_result = match order.kind.as_str() {
        "prepaid" => provider
            .create_prepaid_order(&order.product_code, &order.user_id)
            .await
            .map(|data| (data.trxid, data.status)),
        _ => provider
            .create_game_order(&order.product_code, &order.user_id, order.zone.as_deref())
            .await
            .map(|data| (data.trxid, data.status)),
    };

    match provider_result {
        Ok((trxid, provider_status)) => {
            let now = crate::now_rfc3339();
            let mut tx = pool.begin().await?;

            sqlx::query(
                "UPDATE orders SET status = ?, provider_trxid = ?, provider_status = ?,
                 submitted_at = ?, submission_lease_until = NULL, updated_at = ? WHERE id = ?",
            )
            .bind(OrderStatus::Processing.as_str())
            .bind(&trxid)
            .bind(&provider_status)
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
            tracing::info!(invoice = %order.invoice, ?trxid, "order submitted to provider");
        }
        Err(AppError::ProviderRejected(message)) => {
            let now = crate::now_rfc3339();
            let mut tx = pool.begin().await?;

            sqlx::query(
                "UPDATE orders SET status = ?, failure_reason = ?,
                 submission_lease_until = NULL, updated_at = ? WHERE id = ?",
            )
            .bind(OrderStatus::RefundPending.as_str())
            .bind(&message)
            .bind(&now)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;

            orders::insert_event(
                &mut *tx,
                order.id,
                Some(OrderStatus::Submitting.as_str()),
                OrderStatus::RefundPending.as_str(),
                Some(&format!("ditolak provider: {message}")),
            )
            .await?;

            tx.commit().await?;
            tracing::warn!(invoice = %order.invoice, %message, "provider rejected order");
        }
        Err(error) => {
            let attempts = order.submit_attempts + 1;
            let now = crate::now_rfc3339();

            if attempts >= 5 {
                sqlx::query(
                    "UPDATE orders SET status = ?, submit_attempts = ?, failure_reason = ?,
                     submission_lease_until = NULL, updated_at = ? WHERE id = ?",
                )
                .bind(OrderStatus::NeedsAttention.as_str())
                .bind(attempts)
                .bind(error.to_string())
                .bind(&now)
                .bind(order.id)
                .execute(pool)
                .await?;

                orders::insert_event(
                    pool,
                    order.id,
                    Some(OrderStatus::Submitting.as_str()),
                    OrderStatus::NeedsAttention.as_str(),
                    Some("gagal mengirim order ke provider, perlu tindakan admin"),
                )
                .await?;

                tracing::error!(invoice = %order.invoice, %error, "order needs attention");
            } else {
                let backoff_seconds = (10i64 * (1i64 << attempts)).min(300);
                let lease = crate::rfc3339_after_seconds(backoff_seconds);

                sqlx::query(
                    "UPDATE orders SET status = ?, submit_attempts = ?, failure_reason = ?,
                     submission_lease_until = ?, updated_at = ? WHERE id = ?",
                )
                .bind(OrderStatus::Paid.as_str())
                .bind(attempts)
                .bind(error.to_string())
                .bind(&lease)
                .bind(&now)
                .bind(order.id)
                .execute(pool)
                .await?;

                tracing::warn!(
                    invoice = %order.invoice,
                    attempt = attempts,
                    %error,
                    "provider submit failed, will retry"
                );
            }
        }
    }

    Ok(())
}

async fn poll_processing(pool: &SqlitePool, provider: &Provider) -> Result<(), AppError> {
    let sql = format!(
        "SELECT {ORDER_COLUMNS} FROM orders
         WHERE status = ? AND provider_trxid IS NOT NULL
         ORDER BY updated_at LIMIT 5"
    );

    let rows = sqlx::query_as::<_, OrderRow>(&sql)
        .bind(OrderStatus::Processing.as_str())
        .fetch_all(pool)
        .await?;

    for order in rows {
        let Some(trxid) = order.provider_trxid.as_deref() else {
            continue;
        };

        let status_result = if order.kind == "prepaid" {
            provider.prepaid_order_status(trxid).await
        } else {
            provider.game_order_status(trxid).await
        };

        let item = match status_result {
            Ok(item) => item,
            Err(error) => {
                tracing::warn!(invoice = %order.invoice, %trxid, %error, "status check failed");
                continue;
            }
        };

        let Some(item) = item else {
            continue;
        };

        let raw_status = item.status.clone().unwrap_or_default();
        let new_status = provider::map_provider_status(&raw_status);
        let now = crate::now_rfc3339();

        match new_status {
            OrderStatus::Success => {
                let mut tx = pool.begin().await?;

                sqlx::query(
                    "UPDATE orders SET status = ?, provider_status = ?,
                     provider_sn = COALESCE(?, provider_sn), completed_at = ?,
                     submission_lease_until = NULL, updated_at = ? WHERE id = ?",
                )
                .bind(OrderStatus::Success.as_str())
                .bind(&raw_status)
                .bind(&item.sn)
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
                tracing::info!(invoice = %order.invoice, %trxid, "order completed");
            }
            OrderStatus::Failed => {
                let mut tx = pool.begin().await?;

                sqlx::query(
                    "UPDATE orders SET status = ?, provider_status = ?, failure_reason = ?,
                     submission_lease_until = NULL, updated_at = ? WHERE id = ?",
                )
                .bind(OrderStatus::RefundPending.as_str())
                .bind(&raw_status)
                .bind(item.note.clone())
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
                tracing::warn!(invoice = %order.invoice, %trxid, status = %raw_status, "order failed");
            }
            _ => {
                let changed = order.provider_status.as_deref() != Some(raw_status.as_str())
                    || item.sn.is_some();

                if changed {
                    sqlx::query(
                        "UPDATE orders SET provider_status = ?,
                         provider_sn = COALESCE(?, provider_sn), updated_at = ? WHERE id = ?",
                    )
                    .bind(&raw_status)
                    .bind(&item.sn)
                    .bind(&now)
                    .bind(order.id)
                    .execute(pool)
                    .await?;
                }
            }
        }
    }

    Ok(())
}
