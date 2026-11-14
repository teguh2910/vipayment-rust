use crate::{catalog, AppState};
use axum::{
    extract::{Path, Request, State},
    middleware::{self, Next},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower_http::{limit::RequestBodyLimitLayer, services::ServeDir, trace::TraceLayer};
use vipayment_shared::{db, format_idr, AppError};

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
struct ProductRow {
    code: String,
    name: String,
    sell_price: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct ResolvedProductRow {
    name: String,
    base_price: i64,
    sell_price: i64,
    available: i64,
}

pub fn router(state: AppState) -> Router {
    let internal =
        Router::new()
            .route("/catalog/resolve/{slug}/{code}", get(resolve))
            .route_layer(middleware::from_fn({
                let auth = state.internal.clone();
                move |request: Request, next: Next| {
                    let auth = auth.clone();
                    async move {
                        vipayment_shared::internal::require_internal(auth, request, next).await
                    }
                }
            }));

    let api = Router::new()
        .route("/health", get(health))
        .route("/games", get(games))
        .route("/games/{slug}/products", get(products))
        .route("/admin/sync-products", post(sync_products))
        .route("/admin/profile", get(profile));

    Router::new()
        .nest("/api/v1", api)
        .nest("/internal", internal)
        .nest_service("/logos", ServeDir::new(state.cfg.logo_dir.clone()))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn health(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "ok": true,
        "provider_enabled": state.provider.enabled(),
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
        .filter(|row| state.cfg.category_allowed(row.category.as_deref()))
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
                "category_label": row.category.as_deref().map(catalog::category_label),
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
    let game = fetch_game(&state.pool, &slug).await?;
    ensure_sellable(&state, &game)?;

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
        "game": game_json(&game),
        "products": products,
    })))
}

/// Internal lookup used by the order service at checkout. Returns the group
/// metadata and the product price in a single roundtrip, and enforces the
/// storefront allowlist and product availability so the order service never
/// needs the catalog database.
pub async fn resolve(
    State(state): State<AppState>,
    Path((slug, code)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    let game = fetch_game(&state.pool, &slug).await?;
    ensure_sellable(&state, &game)?;

    let product = sqlx::query_as::<_, ResolvedProductRow>(
        "SELECT name, base_price, sell_price, available
         FROM products WHERE game_slug = ? AND code = ?",
    )
    .bind(&slug)
    .bind(&code)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("produk tidak ditemukan".into()))?;

    if product.available == 0 {
        return Err(AppError::Conflict("produk sedang tidak tersedia".into()));
    }

    Ok(Json(json!({
        "game": game_json(&game),
        "product": {
            "code": code,
            "name": product.name,
            "base_price": product.base_price,
            "sell_price": product.sell_price,
        },
    })))
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
            .signature_matches(state.cfg.provider.sign.as_deref()),
        "markup_percent": state.cfg.markup_percent,
        "markup_by_category": &state.cfg.markup_by_category,
        "markup_fixed_by_category": &state.cfg.markup_fixed_by_category,
    })))
}

async fn fetch_game(pool: &SqlitePool, slug: &str) -> Result<GameRow, AppError> {
    sqlx::query_as::<_, GameRow>(
        "SELECT slug, name, icon_url, requires_zone, user_id_label, zone_label, kind, category
         FROM games WHERE slug = ? AND active = 1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("game tidak ditemukan".into()))
}

fn ensure_sellable(state: &AppState, game: &GameRow) -> Result<(), AppError> {
    if !state.cfg.category_allowed(game.category.as_deref()) {
        return Err(AppError::NotFound("game tidak ditemukan".into()));
    }
    Ok(())
}

fn game_json(game: &GameRow) -> Value {
    json!({
        "slug": game.slug,
        "name": game.name,
        "icon_url": game.icon_url,
        "requires_zone": game.requires_zone != 0,
        "user_id_label": game.user_id_label,
        "zone_label": game.zone_label,
        "kind": game.kind,
        "category": game.category,
        "category_label": game.category.as_deref().map(catalog::category_label),
    })
}
