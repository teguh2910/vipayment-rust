use crate::{db, error::AppError, format_idr, orders, uploads, AppState};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, sqlx::FromRow)]
struct GameRow {
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
struct GameSummaryRow {
    slug: String,
    name: String,
    icon_url: Option<String>,
    requires_zone: i64,
    user_id_label: String,
    zone_label: String,
    kind: String,
    category: Option<String>,
    product_count: i64,
    min_price: Option<i64>,
}

#[derive(Debug, sqlx::FromRow)]
struct ProductRow {
    code: String,
    name: String,
    sell_price: i64,
}

pub async fn health(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "ok": true,
        "provider_enabled": state.provider.enabled(),
        "payment_provider": state.payments.name(),
        "last_sync_at": db::get_state(&state.pool, "last_sync_at").await,
        "last_sync_error": db::get_state(&state.pool, "last_sync_error").await,
        "logos_cached": count_logos(&state.cfg.logo_dir).await,
    })))
}

async fn count_logos(dir: &str) -> u64 {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return 0;
    };

    let mut count = 0;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry
            .file_type()
            .await
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            count += 1;
        }
    }
    count
}

pub async fn games(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let rows = sqlx::query_as::<_, GameSummaryRow>(
        "SELECT g.slug, g.name, g.icon_url, g.requires_zone, g.user_id_label, g.zone_label,
                g.kind, g.category,
                (SELECT COUNT(*) FROM products p
                  WHERE p.game_slug = g.slug AND p.available = 1) AS product_count,
                (SELECT MIN(p.sell_price) FROM products p
                  WHERE p.game_slug = g.slug AND p.available = 1) AS min_price
         FROM games g
         WHERE g.active = 1
         ORDER BY g.sort_order, g.name",
    )
    .fetch_all(&state.pool)
    .await?;

    let games: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "slug": row.slug,
                "name": row.name,
                "icon_url": row.icon_url,
                "requires_zone": row.requires_zone != 0,
                "user_id_label": row.user_id_label,
                "zone_label": row.zone_label,
                "kind": row.kind,
                "category": row.category,
                "category_label": row.category.as_deref().map(crate::catalog::category_label),
                "product_count": row.product_count,
                "min_price": row.min_price,
                "min_price_label": row.min_price.map(format_idr),
            })
        })
        .collect();

    Ok(Json(json!({ "games": games })))
}

pub async fn products(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<Value>, AppError> {
    let game = sqlx::query_as::<_, GameRow>(
        "SELECT slug, name, icon_url, requires_zone, user_id_label, zone_label, kind, category
         FROM games WHERE slug = ? AND active = 1",
    )
    .bind(&slug)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("game tidak ditemukan".into()))?;

    let rows = sqlx::query_as::<_, ProductRow>(
        "SELECT code, name, sell_price FROM products
         WHERE game_slug = ? AND available = 1
         ORDER BY sell_price, name",
    )
    .bind(&slug)
    .fetch_all(&state.pool)
    .await?;

    let products: Vec<Value> = rows
        .into_iter()
        .map(|row| {
            json!({
                "code": row.code,
                "name": row.name,
                "sell_price": row.sell_price,
                "price_label": format_idr(row.sell_price),
            })
        })
        .collect();

    Ok(Json(json!({
        "game": {
            "slug": game.slug,
            "name": game.name,
            "icon_url": game.icon_url,
            "requires_zone": game.requires_zone != 0,
            "user_id_label": game.user_id_label,
            "zone_label": game.zone_label,
            "kind": game.kind,
            "category": game.category,
            "category_label": game.category.as_deref().map(crate::catalog::category_label),
        },
        "products": products,
    })))
}

#[derive(Debug, Deserialize)]
pub struct LookupQuery {
    pub contact: String,
}

pub async fn lookup_orders(
    State(state): State<AppState>,
    Query(query): Query<LookupQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        orders::lookup_orders_by_contact(&state.pool, &query.contact).await?,
    ))
}

pub async fn create_order(
    State(state): State<AppState>,
    Json(request): Json<orders::CreateOrderRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    if !state.provider.enabled() {
        return Err(AppError::BadRequest(
            "layanan top-up sedang dinonaktifkan".into(),
        ));
    }

    let view =
        orders::create_order(&state.pool, &state.cfg, state.payments.as_ref(), request).await?;

    Ok((StatusCode::CREATED, Json(view)))
}

pub async fn order_detail(
    State(state): State<AppState>,
    Path(invoice): Path<String>,
) -> Result<Json<Value>, AppError> {
    let view =
        orders::order_detail(&state.pool, &state.cfg, state.payments.as_ref(), &invoice).await?;

    Ok(Json(view))
}

pub async fn upload_proof(
    State(state): State<AppState>,
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
