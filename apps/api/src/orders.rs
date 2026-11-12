use crate::{
    config::Config,
    error::AppError,
    format_idr, mask_contact, now_rfc3339,
    payments::PaymentProvider,
    rfc3339_after_minutes, rfc3339_after_seconds,
    status::{OrderStatus, PaymentStatus},
};
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

pub const ORDER_COLUMNS: &str = "id, invoice, game_slug, product_code, product_name, user_id, zone, contact, \
base_price, sell_price, amount, status, provider_trxid, provider_status, provider_sn, submit_attempts, \
submission_lease_until, failure_reason, created_at, updated_at, paid_at, submitted_at, completed_at, kind";

pub const PAYMENT_COLUMNS: &str = "id, order_id, provider, method, amount, status, proof_path, \
proof_uploaded_at, reviewed_by, reviewed_at, reject_reason, expires_at, created_at";

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrderRow {
    pub id: i64,
    pub invoice: String,
    pub game_slug: String,
    pub product_code: String,
    pub product_name: String,
    pub user_id: String,
    pub zone: Option<String>,
    pub contact: String,
    pub base_price: i64,
    pub sell_price: i64,
    pub amount: i64,
    pub status: String,
    pub provider_trxid: Option<String>,
    pub provider_status: Option<String>,
    pub provider_sn: Option<String>,
    pub submit_attempts: i64,
    pub submission_lease_until: Option<String>,
    pub failure_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub paid_at: Option<String>,
    pub submitted_at: Option<String>,
    pub completed_at: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PaymentRow {
    pub id: i64,
    pub order_id: i64,
    pub provider: String,
    pub method: String,
    pub amount: i64,
    pub status: String,
    pub proof_path: Option<String>,
    pub proof_uploaded_at: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<String>,
    pub reject_reason: Option<String>,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct GameMetaRow {
    slug: String,
    name: String,
    icon_url: Option<String>,
    requires_zone: i64,
    user_id_label: String,
    zone_label: String,
    kind: String,
    category: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct EventRow {
    from_status: Option<String>,
    to_status: String,
    note: Option<String>,
    created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ProductRow {
    name: String,
    base_price: i64,
    sell_price: i64,
    available: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    pub game_slug: String,
    pub product_code: String,
    pub user_id: String,
    #[serde(default)]
    pub zone: Option<String>,
    pub contact: String,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

pub async fn find_order(pool: &SqlitePool, invoice: &str) -> Result<Option<OrderRow>, AppError> {
    let sql = format!("SELECT {ORDER_COLUMNS} FROM orders WHERE invoice = ?");
    Ok(sqlx::query_as::<_, OrderRow>(&sql)
        .bind(invoice)
        .fetch_optional(pool)
        .await?)
}

pub async fn latest_payment(
    pool: &SqlitePool,
    order_id: i64,
) -> Result<Option<PaymentRow>, AppError> {
    let sql = format!(
        "SELECT {PAYMENT_COLUMNS} FROM payments WHERE order_id = ? ORDER BY id DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, PaymentRow>(&sql)
        .bind(order_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn insert_event<'e, E>(
    executor: E,
    order_id: i64,
    from_status: Option<&str>,
    to_status: &str,
    note: Option<&str>,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO order_events (order_id, from_status, to_status, note, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(order_id)
    .bind(from_status)
    .bind(to_status)
    .bind(note)
    .bind(now_rfc3339())
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn create_order(
    pool: &SqlitePool,
    cfg: &Config,
    payments: &dyn PaymentProvider,
    request: CreateOrderRequest,
) -> Result<Value, AppError> {
    let game_slug = request.game_slug.trim().to_string();
    let product_code = request.product_code.trim().to_string();
    let user_id = request.user_id.trim().to_string();
    let contact = request.contact.trim().to_string();
    let mut zone = request
        .zone
        .as_deref()
        .map(str::trim)
        .filter(|zone| !zone.is_empty())
        .map(str::to_string);

    validate_field("game_slug", &game_slug, 64)?;
    validate_field("product_code", &product_code, 64)?;
    validate_field("user_id", &user_id, 64)?;
    validate_field("contact", &contact, 64)?;
    if let Some(zone) = &zone {
        validate_field("zone", zone, 32)?;
    }
    if !looks_like_contact(&contact) {
        return Err(AppError::BadRequest(
            "kontak harus nomor WhatsApp atau email yang valid".into(),
        ));
    }

    let game = sqlx::query_as::<_, GameMetaRow>(
        "SELECT slug, name, icon_url, requires_zone, user_id_label, zone_label, kind, category
         FROM games WHERE slug = ? AND active = 1",
    )
    .bind(&game_slug)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("produk tidak ditemukan".into()))?;

    // Only game top-ups accept a zone/server; prepaid and app products do not.
    if game.kind != "game" {
        zone = None;
    }

    if game.kind == "game" && game.requires_zone != 0 && zone.is_none() {
        return Err(AppError::BadRequest(format!(
            "{} wajib diisi untuk game ini",
            game.zone_label
        )));
    }

    let product = sqlx::query_as::<_, ProductRow>(
        "SELECT name, base_price, sell_price, available
         FROM products WHERE code = ? AND game_slug = ?",
    )
    .bind(&product_code)
    .bind(&game_slug)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("produk tidak ditemukan".into()))?;

    if product.available == 0 {
        return Err(AppError::Conflict("produk sedang tidak tersedia".into()));
    }
    if product.sell_price <= 0 || product.sell_price > cfg.max_order_amount {
        return Err(AppError::BadRequest("nominal order di luar batas".into()));
    }

    let idempotency_key = request
        .idempotency_key
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty());
    if let Some(key) = &idempotency_key {
        if key.len() > 128 {
            return Err(AppError::BadRequest(
                "idempotency_key terlalu panjang".into(),
            ));
        }
        if let Some(invoice) =
            sqlx::query_scalar::<_, String>("SELECT invoice FROM orders WHERE idempotency_key = ?")
                .bind(key)
                .fetch_optional(pool)
                .await?
        {
            return order_detail(pool, cfg, payments, &invoice).await;
        }
    }

    let window_start = rfc3339_after_seconds(-600);
    let recent_orders: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE contact = ? AND created_at > ?")
            .bind(&contact)
            .bind(&window_start)
            .fetch_one(pool)
            .await?;
    if recent_orders >= 8 {
        return Err(AppError::TooManyRequests(
            "terlalu banyak order dari kontak ini, coba lagi beberapa menit lagi".into(),
        ));
    }

    let expires_at = rfc3339_after_minutes(cfg.payment_expiry_minutes);
    let now = now_rfc3339();
    let mut tx = pool.begin().await?;

    let mut invoice = String::new();
    for _ in 0..5 {
        let candidate = generate_invoice();
        let exists =
            sqlx::query_scalar::<_, String>("SELECT invoice FROM orders WHERE invoice = ?")
                .bind(&candidate)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            invoice = candidate;
            break;
        }
    }
    if invoice.is_empty() {
        return Err(AppError::Internal("gagal membuat nomor invoice".into()));
    }

    let insert = sqlx::query(
        "INSERT INTO orders
            (invoice, idempotency_key, game_slug, product_code, product_name, user_id, zone,
             contact, base_price, sell_price, amount, status, kind, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&invoice)
    .bind(&idempotency_key)
    .bind(&game_slug)
    .bind(&product_code)
    .bind(&product.name)
    .bind(&user_id)
    .bind(&zone)
    .bind(&contact)
    .bind(product.base_price)
    .bind(product.sell_price)
    .bind(product.sell_price)
    .bind(OrderStatus::PendingPayment.as_str())
    .bind(&game.kind)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await;

    let order_id = match insert {
        Ok(result) => result.last_insert_rowid(),
        Err(sqlx::Error::Database(db_error)) if db_error.is_unique_violation() => {
            tx.rollback().await.ok();
            if let Some(key) = &idempotency_key {
                if let Some(existing) = sqlx::query_scalar::<_, String>(
                    "SELECT invoice FROM orders WHERE idempotency_key = ?",
                )
                .bind(key)
                .fetch_optional(pool)
                .await?
                {
                    return order_detail(pool, cfg, payments, &existing).await;
                }
            }
            return Err(AppError::Conflict(
                "invoice bentrok, silakan coba lagi".into(),
            ));
        }
        Err(error) => return Err(error.into()),
    };

    sqlx::query(
        "INSERT INTO payments (order_id, provider, method, amount, status, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(order_id)
    .bind(payments.name())
    .bind("pending")
    .bind(product.sell_price)
    .bind(PaymentStatus::Pending.as_str())
    .bind(&expires_at)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    insert_event(
        &mut *tx,
        order_id,
        None,
        OrderStatus::PendingPayment.as_str(),
        Some("order dibuat, menunggu pembayaran"),
    )
    .await?;

    tx.commit().await?;

    let instruction = payments
        .create_payment(&invoice, product.sell_price, &expires_at)
        .await?;
    sqlx::query("UPDATE payments SET method = ? WHERE order_id = ?")
        .bind(&instruction.method)
        .bind(order_id)
        .execute(pool)
        .await?;

    order_detail(pool, cfg, payments, &invoice).await
}

pub async fn order_detail(
    pool: &SqlitePool,
    _cfg: &Config,
    payments: &dyn PaymentProvider,
    invoice: &str,
) -> Result<Value, AppError> {
    let order = find_order(pool, invoice)
        .await?
        .ok_or_else(|| AppError::NotFound("invoice tidak ditemukan".into()))?;
    let payment = latest_payment(pool, order.id).await?;

    let game = sqlx::query_as::<_, GameMetaRow>(
        "SELECT slug, name, icon_url, requires_zone, user_id_label, zone_label, kind, category
         FROM games WHERE slug = ?",
    )
    .bind(&order.game_slug)
    .fetch_optional(pool)
    .await?;

    let events = sqlx::query_as::<_, EventRow>(
        "SELECT from_status, to_status, note, created_at
         FROM order_events WHERE order_id = ? ORDER BY id",
    )
    .bind(order.id)
    .fetch_all(pool)
    .await?;

    let order_status = OrderStatus::parse(&order.status).unwrap_or(OrderStatus::PendingPayment);
    let payment_status = payment
        .as_ref()
        .and_then(|row| PaymentStatus::parse(&row.status));

    let instructions = match (&payment, order_status) {
        (Some(row), OrderStatus::PendingPayment)
            if matches!(
                row.status.as_str(),
                "pending" | "awaiting_review" | "rejected"
            ) =>
        {
            Some(
                payments
                    .create_payment(&order.invoice, order.amount, &row.expires_at)
                    .await?,
            )
        }
        _ => None,
    };

    let user_id_label = game
        .as_ref()
        .map(|game| game.user_id_label.clone())
        .unwrap_or_else(|| "User ID".to_string());
    let zone_label = game
        .as_ref()
        .map(|game| game.zone_label.clone())
        .unwrap_or_else(|| "Zone ID".to_string());

    Ok(json!({
        "invoice": order.invoice,
        "status": order.status,
        "amount": order.amount,
        "amount_label": format_idr(order.amount),
        "created_at": order.created_at,
        "paid_at": order.paid_at,
        "completed_at": order.completed_at,
        "game": game.as_ref().map(|game| json!({
            "slug": game.slug,
            "name": game.name,
            "icon_url": game.icon_url,
            "requires_zone": game.requires_zone != 0,
            "kind": game.kind,
            "category": game.category,
            "category_label": game.category.as_deref().map(crate::catalog::category_label),
        })),
        "product": {
            "code": order.product_code,
            "name": order.product_name,
        },
        "account": {
            "user_id": order.user_id,
            "zone": order.zone,
            "user_id_label": user_id_label,
            "zone_label": zone_label,
        },
        "contact": mask_contact(&order.contact),
        "provider": {
            "trxid": order.provider_trxid,
            "status": order.provider_status,
            "sn": order.provider_sn,
            "failure_reason": order.failure_reason,
        },
        "payment": payment.map(|row| json!({
            "status": row.status,
            "status_kind": payment_status.map(|status| status.as_str()),
            "provider": row.provider,
            "method": row.method,
            "amount": row.amount,
            "proof_url": row.proof_path.map(|path| format!("/uploads/{path}")),
            "proof_uploaded_at": row.proof_uploaded_at,
            "expires_at": row.expires_at,
            "reject_reason": row.reject_reason,
            "instructions": instructions,
        })),
        "events": events.into_iter().map(|event| json!({
            "from": event.from_status,
            "to": event.to_status,
            "note": event.note,
            "at": event.created_at,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn ensure_can_upload(pool: &SqlitePool, invoice: &str) -> Result<OrderRow, AppError> {
    let order = find_order(pool, invoice)
        .await?
        .ok_or_else(|| AppError::NotFound("invoice tidak ditemukan".into()))?;

    if order.status != OrderStatus::PendingPayment.as_str() {
        return Err(AppError::Conflict(
            "order ini sudah tidak menunggu pembayaran".into(),
        ));
    }

    let payment = latest_payment(pool, order.id)
        .await?
        .ok_or_else(|| AppError::Internal("data pembayaran tidak ditemukan".into()))?;

    if !matches!(
        payment.status.as_str(),
        "pending" | "awaiting_review" | "rejected"
    ) {
        return Err(AppError::Conflict(
            "pembayaran ini tidak dapat diubah lagi".into(),
        ));
    }

    if payment.status == PaymentStatus::Pending.as_str() && payment.expires_at < now_rfc3339() {
        return Err(AppError::Conflict(
            "batas waktu pembayaran sudah lewat".into(),
        ));
    }

    Ok(order)
}

pub async fn attach_proof(
    pool: &SqlitePool,
    order_id: i64,
    filename: &str,
) -> Result<(), AppError> {
    let now = now_rfc3339();
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE payments SET status = ?, proof_path = ?, proof_uploaded_at = ?, reject_reason = NULL
         WHERE order_id = ? AND status IN ('pending', 'awaiting_review', 'rejected')",
    )
    .bind(PaymentStatus::AwaitingReview.as_str())
    .bind(filename)
    .bind(&now)
    .bind(order_id)
    .execute(&mut *tx)
    .await?;

    insert_event(
        &mut *tx,
        order_id,
        Some(OrderStatus::PendingPayment.as_str()),
        OrderStatus::PendingPayment.as_str(),
        Some("bukti pembayaran diunggah, menunggu verifikasi admin"),
    )
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn approve_payment(
    pool: &SqlitePool,
    cfg: &Config,
    payments: &dyn PaymentProvider,
    invoice: &str,
) -> Result<Value, AppError> {
    let order = find_order(pool, invoice)
        .await?
        .ok_or_else(|| AppError::NotFound("invoice tidak ditemukan".into()))?;
    let payment = latest_payment(pool, order.id)
        .await?
        .ok_or_else(|| AppError::Internal("data pembayaran tidak ditemukan".into()))?;

    if payment.status == PaymentStatus::Paid.as_str() {
        return order_detail(pool, cfg, payments, invoice).await;
    }

    if matches!(order.status.as_str(), "expired" | "refunded") {
        return Err(AppError::Conflict(
            "order ini tidak dapat dikonfirmasi pembayarannya".into(),
        ));
    }

    let now = now_rfc3339();
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE payments SET status = ?, reviewed_by = ?, reviewed_at = ?, reject_reason = NULL
         WHERE id = ?",
    )
    .bind(PaymentStatus::Paid.as_str())
    .bind("admin")
    .bind(&now)
    .bind(payment.id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE orders SET status = ?, paid_at = COALESCE(paid_at, ?), updated_at = ?,
         submission_lease_until = NULL WHERE id = ?",
    )
    .bind(OrderStatus::Paid.as_str())
    .bind(&now)
    .bind(&now)
    .bind(order.id)
    .execute(&mut *tx)
    .await?;

    insert_event(
        &mut *tx,
        order.id,
        Some(&order.status),
        OrderStatus::Paid.as_str(),
        Some("pembayaran dikonfirmasi admin"),
    )
    .await?;

    tx.commit().await?;
    order_detail(pool, cfg, payments, invoice).await
}

pub async fn reject_payment(
    pool: &SqlitePool,
    cfg: &Config,
    payments: &dyn PaymentProvider,
    invoice: &str,
    reason: &str,
) -> Result<Value, AppError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(AppError::BadRequest("alasan penolakan wajib diisi".into()));
    }

    let order = find_order(pool, invoice)
        .await?
        .ok_or_else(|| AppError::NotFound("invoice tidak ditemukan".into()))?;
    let payment = latest_payment(pool, order.id)
        .await?
        .ok_or_else(|| AppError::Internal("data pembayaran tidak ditemukan".into()))?;

    if payment.status != PaymentStatus::AwaitingReview.as_str() {
        return Err(AppError::Conflict(
            "tidak ada bukti pembayaran yang menunggu review".into(),
        ));
    }

    let now = now_rfc3339();
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE payments SET status = ?, reviewed_by = ?, reviewed_at = ?, reject_reason = ?
         WHERE id = ?",
    )
    .bind(PaymentStatus::Rejected.as_str())
    .bind("admin")
    .bind(&now)
    .bind(reason)
    .bind(payment.id)
    .execute(&mut *tx)
    .await?;

    insert_event(
        &mut *tx,
        order.id,
        Some(&order.status),
        &order.status,
        Some(&format!("bukti pembayaran ditolak: {reason}")),
    )
    .await?;

    tx.commit().await?;
    order_detail(pool, cfg, payments, invoice).await
}

pub async fn retry_order(
    pool: &SqlitePool,
    cfg: &Config,
    payments: &dyn PaymentProvider,
    invoice: &str,
) -> Result<Value, AppError> {
    let order = find_order(pool, invoice)
        .await?
        .ok_or_else(|| AppError::NotFound("invoice tidak ditemukan".into()))?;

    if !matches!(
        order.status.as_str(),
        "needs_attention" | "failed" | "refund_pending"
    ) {
        return Err(AppError::Conflict(
            "order ini tidak dalam status yang bisa diulang".into(),
        ));
    }

    let now = now_rfc3339();
    sqlx::query(
        "UPDATE orders SET status = ?, submit_attempts = 0, submission_lease_until = NULL,
         failure_reason = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(OrderStatus::Paid.as_str())
    .bind(&now)
    .bind(order.id)
    .execute(pool)
    .await?;

    insert_event(
        pool,
        order.id,
        Some(&order.status),
        OrderStatus::Paid.as_str(),
        Some("order dikembalikan ke antrean oleh admin"),
    )
    .await?;

    order_detail(pool, cfg, payments, invoice).await
}

pub async fn mark_refunded(
    pool: &SqlitePool,
    cfg: &Config,
    payments: &dyn PaymentProvider,
    invoice: &str,
) -> Result<Value, AppError> {
    let order = find_order(pool, invoice)
        .await?
        .ok_or_else(|| AppError::NotFound("invoice tidak ditemukan".into()))?;

    if order.status != OrderStatus::RefundPending.as_str() {
        return Err(AppError::Conflict(
            "order ini tidak sedang menunggu refund".into(),
        ));
    }

    let now = now_rfc3339();
    sqlx::query("UPDATE orders SET status = ?, updated_at = ? WHERE id = ?")
        .bind(OrderStatus::Refunded.as_str())
        .bind(&now)
        .bind(order.id)
        .execute(pool)
        .await?;

    insert_event(
        pool,
        order.id,
        Some(OrderStatus::RefundPending.as_str()),
        OrderStatus::Refunded.as_str(),
        Some("refund ditandai selesai oleh admin"),
    )
    .await?;

    order_detail(pool, cfg, payments, invoice).await
}

#[derive(Debug, sqlx::FromRow)]
struct AdminPaymentRow {
    invoice: String,
    game_slug: String,
    product_name: String,
    user_id: String,
    zone: Option<String>,
    contact: String,
    amount: i64,
    order_status: String,
    provider_trxid: Option<String>,
    provider_status: Option<String>,
    payment_status: String,
    method: String,
    proof_path: Option<String>,
    proof_uploaded_at: Option<String>,
    expires_at: String,
    reject_reason: Option<String>,
    created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LookupRow {
    invoice: String,
    product_name: String,
    amount: i64,
    status: String,
    created_at: String,
}

pub async fn lookup_orders_by_contact(pool: &SqlitePool, contact: &str) -> Result<Value, AppError> {
    let contact = contact.trim();
    validate_field("kontak", contact, 64)?;

    let rows = if contact.contains('@') {
        sqlx::query_as::<_, LookupRow>(
            "SELECT invoice, product_name, amount, status, created_at FROM orders
             WHERE lower(contact) = lower(?) ORDER BY id DESC LIMIT 20",
        )
        .bind(contact)
        .fetch_all(pool)
        .await?
    } else {
        let variants = crate::util::phone_variants(contact);
        let Some(first) = variants.first() else {
            return Err(AppError::BadRequest(
                "kontak harus nomor WhatsApp atau email yang valid".into(),
            ));
        };
        if first.len() < 8 {
            return Err(AppError::BadRequest(
                "kontak harus nomor WhatsApp atau email yang valid".into(),
            ));
        }
        let second = variants.get(1).unwrap_or(first);

        sqlx::query_as::<_, LookupRow>(
            "SELECT invoice, product_name, amount, status, created_at FROM orders
             WHERE replace(replace(replace(replace(contact, ' ', ''), '-', ''), '+', ''), '.', '')
                   IN (?, ?)
             ORDER BY id DESC LIMIT 20",
        )
        .bind(first)
        .bind(second)
        .fetch_all(pool)
        .await?
    };

    Ok(json!({
        "orders": rows.into_iter().map(|row| json!({
            "invoice": row.invoice,
            "product_name": row.product_name,
            "amount": row.amount,
            "amount_label": format_idr(row.amount),
            "status": row.status,
            "created_at": row.created_at,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn list_payments(
    pool: &SqlitePool,
    status: Option<&str>,
    search: Option<&str>,
) -> Result<Value, AppError> {
    let filter = status.unwrap_or("awaiting_review");
    let pattern = search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"));

    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT o.invoice, o.game_slug, o.product_name, o.user_id, o.zone, o.contact,
                o.amount, o.status AS order_status, o.provider_trxid, o.provider_status,
                p.status AS payment_status, p.method, p.proof_path, p.proof_uploaded_at,
                p.expires_at, p.reject_reason, p.created_at
         FROM payments p JOIN orders o ON o.id = p.order_id
         WHERE 1 = 1",
    );

    if filter != "all" {
        builder.push(" AND p.status = ").push_bind(filter);
    }

    if let Some(pattern) = pattern.as_deref() {
        builder
            .push(" AND (o.invoice LIKE ")
            .push_bind(pattern)
            .push(" OR o.product_name LIKE ")
            .push_bind(pattern)
            .push(" OR o.user_id LIKE ")
            .push_bind(pattern)
            .push(" OR o.contact LIKE ")
            .push_bind(pattern)
            .push(" OR o.provider_trxid LIKE ")
            .push_bind(pattern)
            .push(")");
    }

    builder.push(" ORDER BY p.id DESC LIMIT 200");
    let rows = builder
        .build_query_as::<AdminPaymentRow>()
        .fetch_all(pool)
        .await?;

    Ok(json!({
        "payments": rows.into_iter().map(|row| json!({
            "invoice": row.invoice,
            "game_slug": row.game_slug,
            "product_name": row.product_name,
            "user_id": row.user_id,
            "zone": row.zone,
            "contact": row.contact,
            "amount": row.amount,
            "amount_label": format_idr(row.amount),
            "order_status": row.order_status,
            "provider_trxid": row.provider_trxid,
            "provider_status": row.provider_status,
            "payment_status": row.payment_status,
            "method": row.method,
            "proof_url": row.proof_path.map(|path| format!("/uploads/{path}")),
            "proof_uploaded_at": row.proof_uploaded_at,
            "expires_at": row.expires_at,
            "reject_reason": row.reject_reason,
            "created_at": row.created_at,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn list_orders(
    pool: &SqlitePool,
    status: Option<&str>,
    search: Option<&str>,
) -> Result<Value, AppError> {
    let filter = status.unwrap_or("all");
    let pattern = search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"));

    let mut builder =
        QueryBuilder::<Sqlite>::new(format!("SELECT {ORDER_COLUMNS} FROM orders WHERE 1 = 1"));

    if filter != "all" {
        builder.push(" AND status = ").push_bind(filter);
    }

    if let Some(pattern) = pattern.as_deref() {
        builder
            .push(" AND (invoice LIKE ")
            .push_bind(pattern)
            .push(" OR product_name LIKE ")
            .push_bind(pattern)
            .push(" OR user_id LIKE ")
            .push_bind(pattern)
            .push(" OR contact LIKE ")
            .push_bind(pattern)
            .push(" OR provider_trxid LIKE ")
            .push_bind(pattern)
            .push(" OR provider_sn LIKE ")
            .push_bind(pattern)
            .push(")");
    }

    builder.push(" ORDER BY id DESC LIMIT 200");
    let rows = builder.build_query_as::<OrderRow>().fetch_all(pool).await?;

    Ok(json!({
        "orders": rows.into_iter().map(|order| json!({
            "invoice": order.invoice,
            "game_slug": order.game_slug,
            "product_name": order.product_name,
            "user_id": order.user_id,
            "zone": order.zone,
            "contact": order.contact,
            "amount": order.amount,
            "status": order.status,
            "provider_trxid": order.provider_trxid,
            "provider_status": order.provider_status,
            "provider_sn": order.provider_sn,
            "submit_attempts": order.submit_attempts,
            "failure_reason": order.failure_reason,
            "created_at": order.created_at,
            "updated_at": order.updated_at,
            "paid_at": order.paid_at,
            "submitted_at": order.submitted_at,
            "completed_at": order.completed_at,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn expire_pending(pool: &SqlitePool) -> Result<u64, AppError> {
    let now = now_rfc3339();

    sqlx::query("UPDATE payments SET status = ? WHERE status = ? AND expires_at < ?")
        .bind(PaymentStatus::Expired.as_str())
        .bind(PaymentStatus::Pending.as_str())
        .bind(&now)
        .execute(pool)
        .await?;

    let sql = format!(
        "SELECT {ORDER_COLUMNS} FROM orders
         WHERE status = ?
           AND EXISTS (SELECT 1 FROM payments p WHERE p.order_id = orders.id AND p.status = ?)
           AND NOT EXISTS (SELECT 1 FROM payments p WHERE p.order_id = orders.id AND p.status IN (?, ?))"
    );

    let rows = sqlx::query_as::<_, OrderRow>(&sql)
        .bind(OrderStatus::PendingPayment.as_str())
        .bind(PaymentStatus::Expired.as_str())
        .bind(PaymentStatus::Paid.as_str())
        .bind(PaymentStatus::AwaitingReview.as_str())
        .fetch_all(pool)
        .await?;

    let mut expired = 0u64;
    for order in rows {
        let updated =
            sqlx::query("UPDATE orders SET status = ?, updated_at = ? WHERE id = ? AND status = ?")
                .bind(OrderStatus::Expired.as_str())
                .bind(&now)
                .bind(order.id)
                .bind(OrderStatus::PendingPayment.as_str())
                .execute(pool)
                .await?;

        if updated.rows_affected() == 1 {
            insert_event(
                pool,
                order.id,
                Some(OrderStatus::PendingPayment.as_str()),
                OrderStatus::Expired.as_str(),
                Some("batas waktu pembayaran terlewati"),
            )
            .await?;
            expired += 1;
        }
    }

    Ok(expired)
}

pub async fn simulation_autopay(pool: &SqlitePool, delay_seconds: i64) -> Result<u64, AppError> {
    let cutoff = rfc3339_after_seconds(-delay_seconds);
    let rows = sqlx::query_as::<_, (i64, String)>(
        "SELECT o.id, o.invoice FROM orders o
         JOIN payments p ON p.order_id = o.id
         WHERE o.status = ? AND p.status = ? AND p.created_at <= ?
         ORDER BY o.id LIMIT 20",
    )
    .bind(OrderStatus::PendingPayment.as_str())
    .bind(PaymentStatus::Pending.as_str())
    .bind(&cutoff)
    .fetch_all(pool)
    .await?;

    let mut paid = 0u64;
    for (order_id, invoice) in rows {
        let now = now_rfc3339();
        let mut tx = pool.begin().await?;

        let updated = sqlx::query(
            "UPDATE payments SET status = ?, reviewed_by = ?, reviewed_at = ?
             WHERE order_id = ? AND status = ?",
        )
        .bind(PaymentStatus::Paid.as_str())
        .bind("simulation")
        .bind(&now)
        .bind(order_id)
        .bind(PaymentStatus::Pending.as_str())
        .execute(&mut *tx)
        .await?;

        if updated.rows_affected() == 0 {
            tx.rollback().await.ok();
            continue;
        }

        sqlx::query(
            "UPDATE orders SET status = ?, paid_at = ?, updated_at = ?,
             submission_lease_until = NULL WHERE id = ? AND status = ?",
        )
        .bind(OrderStatus::Paid.as_str())
        .bind(&now)
        .bind(&now)
        .bind(order_id)
        .bind(OrderStatus::PendingPayment.as_str())
        .execute(&mut *tx)
        .await?;

        insert_event(
            &mut *tx,
            order_id,
            Some(OrderStatus::PendingPayment.as_str()),
            OrderStatus::Paid.as_str(),
            Some("pembayaran simulasi dikonfirmasi otomatis"),
        )
        .await?;

        tx.commit().await?;
        paid += 1;
        tracing::info!(%invoice, "simulation payment auto-confirmed");
    }

    Ok(paid)
}

pub fn generate_invoice() -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

    let date = chrono::Utc::now().format("%y%m%d");
    let mut rng = rand::thread_rng();
    let suffix: String = (0..8)
        .map(|_| {
            let index = rng.gen_range(0..ALPHABET.len());
            ALPHABET[index] as char
        })
        .collect();

    format!("INV-{date}-{suffix}")
}

fn validate_field(name: &str, value: &str, max_len: usize) -> Result<(), AppError> {
    if value.is_empty() {
        return Err(AppError::BadRequest(format!("{name} wajib diisi")));
    }
    if value.chars().count() > max_len {
        return Err(AppError::BadRequest(format!("{name} terlalu panjang")));
    }
    Ok(())
}

fn looks_like_contact(value: &str) -> bool {
    if value.contains('@') {
        let mut parts = value.split('@');
        let local = parts.next().unwrap_or_default();
        let domain = parts.next().unwrap_or_default();
        return !local.is_empty() && domain.contains('.') && parts.next().is_none();
    }

    value.chars().filter(|c| c.is_ascii_digit()).count() >= 8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_invoice_with_expected_shape() {
        let invoice = generate_invoice();
        let parts: Vec<&str> = invoice.split('-').collect();

        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "INV");
        assert_eq!(parts[1].len(), 6);
        assert_eq!(parts[2].len(), 8);
        assert!(parts[2]
            .chars()
            .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c)));
    }

    #[test]
    fn generates_distinct_invoices() {
        let first = generate_invoice();
        let second = generate_invoice();
        assert_ne!(first, second);
    }

    #[test]
    fn accepts_phone_numbers_and_emails_only() {
        assert!(looks_like_contact("081234567890"));
        assert!(looks_like_contact("+62 812 3456 7890"));
        assert!(looks_like_contact("teguh@example.com"));
        assert!(!looks_like_contact("123"));
        assert!(!looks_like_contact("teguh@@example.com"));
        assert!(!looks_like_contact("teguh@localhost"));
    }
}
