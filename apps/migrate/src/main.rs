//! Copies data from the legacy monolithic database into the per-service
//! databases. Safe to re-run: every insert is idempotent (`ON CONFLICT`).

use sqlx::{Row, SqlitePool};
use std::process::ExitCode;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vipayment_shared::config::var_or;
use vipayment_shared::db;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Counts {
    games: i64,
    products: i64,
    orders: i64,
    payments: i64,
    order_events: i64,
}

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "migration failed");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let source_url = var_or("SOURCE_DATABASE_URL", "sqlite://data/app.db");
    let catalog_url = var_or("CATALOG_DATABASE_URL", "sqlite://data/catalog.db");
    let orders_url = var_or("ORDERS_DATABASE_URL", "sqlite://data/orders.db");

    if source_url == catalog_url || source_url == orders_url {
        return Err("SOURCE_DATABASE_URL harus berbeda dari database tujuan".into());
    }

    // The source is opened read-only so a failed migration cannot damage it.
    let source = open_read_only(&source_url).await?;
    let catalog = open_target(&catalog_url).await?;
    vipayment_catalog::migrate(&catalog).await?;

    let orders = open_target(&orders_url).await?;
    vipayment_order::migrate(&orders).await?;

    let before = count_source(&source).await?;
    tracing::info!(?before, "source database counted");

    migrate_catalog(&source, &catalog).await?;
    let backfilled = migrate_orders(&source, &orders).await?;
    tracing::info!(orders_backfilled = backfilled, "orders migrated");

    let catalog_counts = count_catalog(&catalog).await?;
    let order_counts = count_orders(&orders).await?;

    verify(&before, &catalog_counts, &order_counts)?;

    tracing::info!(
        games = catalog_counts.games,
        products = catalog_counts.products,
        orders = order_counts.orders,
        payments = order_counts.payments,
        order_events = order_counts.order_events,
        "migration finished; verify counts and keep the source database as backup"
    );

    Ok(())
}

async fn open_read_only(url: &str) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("{url}?mode=ro"))
        .await?;
    Ok(pool)
}

/// Creates the target database file. Schema migrations are applied by the
/// owning service crate right after this returns.
async fn open_target(url: &str) -> Result<SqlitePool, Box<dyn std::error::Error>> {
    db::ensure_database_dir(url)?;
    let pool = db::connect(url).await?;
    Ok(pool)
}

async fn count_source(pool: &SqlitePool) -> Result<Counts, sqlx::Error> {
    Ok(Counts {
        games: count(pool, "games").await?,
        products: count(pool, "products").await?,
        orders: count(pool, "orders").await?,
        payments: count(pool, "payments").await?,
        order_events: count(pool, "order_events").await?,
    })
}

async fn count_catalog(pool: &SqlitePool) -> Result<Counts, sqlx::Error> {
    Ok(Counts {
        games: count(pool, "games").await?,
        products: count(pool, "products").await?,
        ..Counts::default()
    })
}

async fn count_orders(pool: &SqlitePool) -> Result<Counts, sqlx::Error> {
    Ok(Counts {
        orders: count(pool, "orders").await?,
        payments: count(pool, "payments").await?,
        order_events: count(pool, "order_events").await?,
        ..Counts::default()
    })
}

async fn count(pool: &SqlitePool, table: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
}

async fn migrate_catalog(source: &SqlitePool, target: &SqlitePool) -> Result<(), sqlx::Error> {
    // Groups first: products reference games(slug).
    let games = sqlx::query(
        "SELECT slug, name, provider_name, icon_url, requires_zone, user_id_label, zone_label,
                sort_order, active, kind, category FROM games",
    )
    .fetch_all(source)
    .await?;

    for row in &games {
        sqlx::query(
            "INSERT INTO games
                (slug, name, provider_name, icon_url, requires_zone, user_id_label, zone_label,
                 sort_order, active, kind, category)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(slug) DO NOTHING",
        )
        .bind(row.get::<String, _>("slug"))
        .bind(row.get::<String, _>("name"))
        .bind(row.get::<String, _>("provider_name"))
        .bind(row.get::<Option<String>, _>("icon_url"))
        .bind(row.get::<i64, _>("requires_zone"))
        .bind(row.get::<String, _>("user_id_label"))
        .bind(row.get::<String, _>("zone_label"))
        .bind(row.get::<i64, _>("sort_order"))
        .bind(row.get::<i64, _>("active"))
        .bind(row.get::<String, _>("kind"))
        .bind(row.get::<Option<String>, _>("category"))
        .execute(target)
        .await?;
    }

    let products = sqlx::query(
        "SELECT game_slug, code, name, brand, base_price, sell_price, provider_status,
                available, updated_at FROM products",
    )
    .fetch_all(source)
    .await?;

    for row in &products {
        sqlx::query(
            "INSERT INTO products
                (game_slug, code, name, brand, base_price, sell_price, provider_status,
                 available, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(game_slug, code) DO NOTHING",
        )
        .bind(row.get::<String, _>("game_slug"))
        .bind(row.get::<String, _>("code"))
        .bind(row.get::<String, _>("name"))
        .bind(row.get::<Option<String>, _>("brand"))
        .bind(row.get::<i64, _>("base_price"))
        .bind(row.get::<i64, _>("sell_price"))
        .bind(row.get::<Option<String>, _>("provider_status"))
        .bind(row.get::<i64, _>("available"))
        .bind(row.get::<String, _>("updated_at"))
        .execute(target)
        .await?;
    }

    tracing::info!(
        games = games.len(),
        products = products.len(),
        "catalog migrated"
    );
    Ok(())
}

/// Copies orders and backfills the denormalized `game_*` columns from the legacy
/// `games` table. Rows whose group has since been deleted keep empty labels but
/// stay readable.
async fn migrate_orders(source: &SqlitePool, target: &SqlitePool) -> Result<usize, sqlx::Error> {
    let orders = sqlx::query(
        "SELECT o.id, o.invoice, o.idempotency_key, o.game_slug,
                COALESCE(g.name, '') AS game_name, g.icon_url AS game_icon_url,
                COALESCE(g.user_id_label, 'User ID') AS game_user_id_label,
                COALESCE(g.zone_label, 'Zone ID') AS game_zone_label,
                g.category AS game_category, COALESCE(g.requires_zone, 0) AS game_requires_zone,
                o.product_code, o.product_name, o.user_id, o.zone, o.contact,
                o.base_price, o.sell_price, o.amount, o.status, o.provider_trxid,
                o.provider_status, o.provider_sn, o.submit_attempts,
                o.submission_lease_until, o.failure_reason, o.created_at, o.updated_at,
                o.paid_at, o.submitted_at, o.completed_at, o.kind
         FROM orders o LEFT JOIN games g ON g.slug = o.game_slug",
    )
    .fetch_all(source)
    .await?;

    for row in &orders {
        sqlx::query(
            "INSERT INTO orders
                (id, invoice, idempotency_key, game_slug, game_name, game_icon_url,
                 game_user_id_label, game_zone_label, game_category, game_requires_zone,
                 product_code, product_name, user_id, zone, contact, base_price, sell_price,
                 amount, status, provider_trxid, provider_status, provider_sn, submit_attempts,
                 submission_lease_until, failure_reason, created_at, updated_at, paid_at,
                 submitted_at, completed_at, kind)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                     ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(row.get::<i64, _>("id"))
        .bind(row.get::<String, _>("invoice"))
        .bind(row.get::<Option<String>, _>("idempotency_key"))
        .bind(row.get::<String, _>("game_slug"))
        .bind(row.get::<String, _>("game_name"))
        .bind(row.get::<Option<String>, _>("game_icon_url"))
        .bind(row.get::<String, _>("game_user_id_label"))
        .bind(row.get::<String, _>("game_zone_label"))
        .bind(row.get::<Option<String>, _>("game_category"))
        .bind(row.get::<i64, _>("game_requires_zone"))
        .bind(row.get::<String, _>("product_code"))
        .bind(row.get::<String, _>("product_name"))
        .bind(row.get::<String, _>("user_id"))
        .bind(row.get::<Option<String>, _>("zone"))
        .bind(row.get::<String, _>("contact"))
        .bind(row.get::<i64, _>("base_price"))
        .bind(row.get::<i64, _>("sell_price"))
        .bind(row.get::<i64, _>("amount"))
        .bind(row.get::<String, _>("status"))
        .bind(row.get::<Option<String>, _>("provider_trxid"))
        .bind(row.get::<Option<String>, _>("provider_status"))
        .bind(row.get::<Option<String>, _>("provider_sn"))
        .bind(row.get::<i64, _>("submit_attempts"))
        .bind(row.get::<Option<String>, _>("submission_lease_until"))
        .bind(row.get::<Option<String>, _>("failure_reason"))
        .bind(row.get::<String, _>("created_at"))
        .bind(row.get::<String, _>("updated_at"))
        .bind(row.get::<Option<String>, _>("paid_at"))
        .bind(row.get::<Option<String>, _>("submitted_at"))
        .bind(row.get::<Option<String>, _>("completed_at"))
        .bind(row.get::<String, _>("kind"))
        .execute(target)
        .await?;
    }

    let payments = sqlx::query(
        "SELECT id, order_id, provider, method, amount, status, proof_path, proof_uploaded_at,
                reviewed_by, reviewed_at, reject_reason, expires_at, created_at FROM payments",
    )
    .fetch_all(source)
    .await?;

    for row in &payments {
        sqlx::query(
            "INSERT INTO payments
                (id, order_id, provider, method, amount, status, proof_path, proof_uploaded_at,
                 reviewed_by, reviewed_at, reject_reason, expires_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(row.get::<i64, _>("id"))
        .bind(row.get::<i64, _>("order_id"))
        .bind(row.get::<String, _>("provider"))
        .bind(row.get::<String, _>("method"))
        .bind(row.get::<i64, _>("amount"))
        .bind(row.get::<String, _>("status"))
        .bind(row.get::<Option<String>, _>("proof_path"))
        .bind(row.get::<Option<String>, _>("proof_uploaded_at"))
        .bind(row.get::<Option<String>, _>("reviewed_by"))
        .bind(row.get::<Option<String>, _>("reviewed_at"))
        .bind(row.get::<Option<String>, _>("reject_reason"))
        .bind(row.get::<String, _>("expires_at"))
        .bind(row.get::<String, _>("created_at"))
        .execute(target)
        .await?;
    }

    let events = sqlx::query(
        "SELECT id, order_id, from_status, to_status, note, created_at FROM order_events",
    )
    .fetch_all(source)
    .await?;

    for row in &events {
        sqlx::query(
            "INSERT INTO order_events
                (id, order_id, from_status, to_status, note, created_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(row.get::<i64, _>("id"))
        .bind(row.get::<i64, _>("order_id"))
        .bind(row.get::<Option<String>, _>("from_status"))
        .bind(row.get::<String, _>("to_status"))
        .bind(row.get::<Option<String>, _>("note"))
        .bind(row.get::<String, _>("created_at"))
        .execute(target)
        .await?;
    }

    // Preserve sync bookkeeping so the catalog service's health endpoint is not
    // blank after the switch, and so order-side state stays intact.
    let states = sqlx::query("SELECT key, value FROM app_state")
        .fetch_all(source)
        .await?;

    for row in &states {
        sqlx::query(
            "INSERT INTO app_state (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO NOTHING",
        )
        .bind(row.get::<String, _>("key"))
        .bind(row.get::<String, _>("value"))
        .execute(target)
        .await?;
    }

    Ok(orders.len())
}

fn verify(
    before: &Counts,
    catalog: &Counts,
    orders: &Counts,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut problems = Vec::new();

    if catalog.games != before.games {
        problems.push(format!(
            "games: source {} vs catalog {}",
            before.games, catalog.games
        ));
    }
    if catalog.products != before.products {
        problems.push(format!(
            "products: source {} vs catalog {}",
            before.products, catalog.products
        ));
    }
    if orders.orders != before.orders {
        problems.push(format!(
            "orders: source {} vs orders {}",
            before.orders, orders.orders
        ));
    }
    if orders.payments != before.payments {
        problems.push(format!(
            "payments: source {} vs orders {}",
            before.payments, orders.payments
        ));
    }
    if orders.order_events != before.order_events {
        problems.push(format!(
            "order_events: source {} vs orders {}",
            before.order_events, orders.order_events
        ));
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("row count mismatch: {}", problems.join("; ")).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_accepts_matching_counts() {
        let counts = Counts {
            games: 2,
            products: 3,
            orders: 4,
            payments: 5,
            order_events: 6,
        };

        assert!(verify(&counts, &counts, &counts).is_ok());
    }

    #[test]
    fn verification_reports_mismatches() {
        let before = Counts {
            games: 2,
            products: 3,
            orders: 4,
            payments: 5,
            order_events: 6,
        };
        let mut after = before.clone();
        after.orders = 99;

        let error = verify(&before, &before, &after).unwrap_err().to_string();
        assert!(error.contains("orders"));
    }
}
