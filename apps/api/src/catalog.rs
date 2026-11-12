use crate::{config::Config, db, error::AppError, provider, provider::Provider};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::Duration;
use vipayment::{GameServiceItem, PrepaidServiceItem};

#[derive(Debug, Clone, Deserialize)]
pub struct GameSeed {
    pub slug: String,
    pub provider_name: String,
    pub name: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub requires_zone: bool,
    #[serde(default)]
    pub user_id_label: Option<String>,
    #[serde(default)]
    pub zone_label: Option<String>,
    #[serde(default)]
    pub sort_order: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct SyncReport {
    pub groups: usize,
    pub products: usize,
    pub logos: usize,
    pub errors: Vec<String>,
}

struct GroupUpsert {
    slug: String,
    name: String,
    provider_name: String,
    icon_url: Option<String>,
    requires_zone: bool,
    user_id_label: String,
    zone_label: String,
    sort_order: i64,
    kind: &'static str,
    category: Option<String>,
}

struct NewProduct {
    code: String,
    name: String,
    brand: Option<String>,
    base_price: i64,
    sell_price: i64,
    provider_status: Option<String>,
    available: bool,
}

/// Resolves and caches group logos. Domains come from `config/logos.json`
/// (provider group name or prepaid brand -> official website domain) and the
/// image itself is fetched once from the Google favicon service, then served
/// from `/logos/...` so the storefront never hotlinks a third party.
struct LogoCache {
    domains: HashMap<String, String>,
    client: Option<reqwest::Client>,
    dir: String,
}

impl LogoCache {
    async fn load(config_path: &str, dir: &str) -> Self {
        let domains = match tokio::fs::read_to_string(config_path).await {
            Ok(raw) => match serde_json::from_str::<HashMap<String, String>>(&raw) {
                Ok(map) => map
                    .into_iter()
                    .map(|(key, value)| (key.to_ascii_lowercase(), value))
                    .collect(),
                Err(error) => {
                    tracing::warn!(%error, path = config_path, "invalid logos config");
                    HashMap::new()
                }
            },
            Err(error) => {
                tracing::debug!(%error, path = config_path, "logos config not found");
                HashMap::new()
            }
        };

        let client = match reqwest::Client::builder()
            .user_agent("vipayment-api/0.1")
            .local_address(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
            .timeout(Duration::from_secs(8))
            .build()
        {
            Ok(client) => Some(client),
            Err(error) => {
                tracing::warn!(%error, "failed to build logo HTTP client");
                None
            }
        };

        Self {
            domains,
            client,
            dir: dir.to_string(),
        }
    }

    async fn resolve(&self, key: &str) -> Option<String> {
        let domain = self.domains.get(&key.to_ascii_lowercase())?;
        let client = self.client.as_ref()?;
        let filename = cached_logo(client, &self.dir, domain).await?;
        Some(format!("/logos/{filename}"))
    }
}

async fn cached_logo(client: &reqwest::Client, dir: &str, domain: &str) -> Option<String> {
    // Logos are cached per domain so groups sharing a site share one file.
    let stem = slugify(domain);

    for extension in ["png", "jpg", "webp"] {
        let filename = format!("{stem}.{extension}");
        if tokio::fs::metadata(Path::new(dir).join(&filename))
            .await
            .is_ok()
        {
            return Some(filename);
        }
    }

    let url = format!("https://www.google.com/s2/favicons?domain={domain}&sz=128");
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(domain, %error, "logo download failed (check server egress)");
            return None;
        }
    };

    if !response.status().is_success() {
        tracing::warn!(domain, status = %response.status(), "logo download rejected");
        return None;
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    let extension = if content_type.contains("png") {
        "png"
    } else if content_type.contains("jpeg") || content_type.contains("jpg") {
        "jpg"
    } else if content_type.contains("webp") {
        "webp"
    } else {
        tracing::warn!(
            domain,
            content_type,
            "logo download has unexpected content type"
        );
        return None;
    };

    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(domain, %error, "logo download body failed");
            return None;
        }
    };
    if bytes.len() < 100 || bytes.len() > 512 * 1024 {
        tracing::warn!(
            domain,
            size = bytes.len(),
            "logo download has unexpected size"
        );
        return None;
    }

    if let Err(error) = tokio::fs::create_dir_all(dir).await {
        tracing::warn!(dir, %error, "cannot create logo directory (check LOGO_DIR permissions)");
        return None;
    }

    let filename = format!("{stem}.{extension}");
    if let Err(error) = tokio::fs::write(Path::new(dir).join(&filename), &bytes).await {
        tracing::warn!(dir, %error, "cannot write logo file (check LOGO_DIR permissions)");
        return None;
    }

    tracing::debug!(domain, "logo cached");
    Some(filename)
}

pub async fn load_seeds(path: &str) -> Result<Vec<GameSeed>, AppError> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| AppError::Internal(format!("gagal membaca {path}: {e}")))?;

    serde_json::from_str(&raw)
        .map_err(|e| AppError::Internal(format!("{path} bukan JSON yang valid: {e}")))
}

pub async fn sync(
    pool: &SqlitePool,
    provider: &Provider,
    cfg: &Config,
) -> Result<SyncReport, AppError> {
    let overrides = load_seeds(&cfg.games_config_path).await?;
    let logos = LogoCache::load(&cfg.logos_config_path, &cfg.logo_dir).await;
    let mut report = SyncReport {
        groups: 0,
        products: 0,
        logos: 0,
        errors: Vec::new(),
    };

    // The provider is the source of groups; anything not seen in this pass is
    // hidden from the storefront (historical orders stay intact).
    sqlx::query("UPDATE games SET active = 0")
        .execute(pool)
        .await?;

    sync_game_feature(pool, provider, cfg, &overrides, &logos, &mut report).await;
    sync_prepaid(pool, provider, cfg, &logos, &mut report).await;

    db::set_state(pool, "last_sync_at", &crate::now_rfc3339()).await;
    if report.errors.is_empty() {
        db::clear_state(pool, "last_sync_error").await;
    } else {
        db::set_state(pool, "last_sync_error", &report.errors.join("; ")).await;
    }

    Ok(report)
}

async fn sync_game_feature(
    pool: &SqlitePool,
    provider: &Provider,
    cfg: &Config,
    overrides: &[GameSeed],
    logos: &LogoCache,
    report: &mut SyncReport,
) {
    let items = match provider.list_game_services(None).await {
        Ok(items) => items,
        Err(error) => {
            tracing::warn!(%error, "game catalog sync failed");
            report.errors.push(format!("game-feature: {error}"));
            return;
        }
    };

    let mut groups: BTreeMap<String, Vec<GameServiceItem>> = BTreeMap::new();
    for item in items {
        if !is_available(item.status.as_deref()) {
            continue;
        }
        let Some(game) = item
            .game
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        groups.entry(game.to_string()).or_default().push(item);
    }

    for (provider_name, items) in &groups {
        let meta = overrides
            .iter()
            .find(|seed| seed.provider_name.eq_ignore_ascii_case(provider_name));

        let kind = if meta.is_some() {
            "game"
        } else if looks_like_app_or_voucher(provider_name) {
            "app"
        } else {
            "game"
        };

        let slug = meta
            .map(|seed| seed.slug.clone())
            .unwrap_or_else(|| format!("g-{}", slugify(provider_name)));
        let markup_percent = cfg.markup_percent_for(kind);
        let icon_url = match meta.and_then(|seed| seed.icon_url.clone()) {
            Some(icon) => Some(icon),
            None => logos.resolve(provider_name).await,
        };
        if icon_url.is_some() {
            report.logos += 1;
        }

        let group = GroupUpsert {
            slug,
            name: meta
                .map(|seed| seed.name.clone())
                .unwrap_or_else(|| provider_name.to_string()),
            provider_name: provider_name.clone(),
            icon_url,
            requires_zone: meta.map(|seed| seed.requires_zone).unwrap_or(false),
            user_id_label: meta
                .and_then(|seed| seed.user_id_label.clone())
                .unwrap_or_else(|| "User ID".to_string()),
            zone_label: meta
                .and_then(|seed| seed.zone_label.clone())
                .unwrap_or_else(|| "Zone / Server".to_string()),
            sort_order: meta.map(|seed| seed.sort_order).unwrap_or(500),
            kind,
            category: None,
        };

        match upsert_group(pool, &group).await {
            Ok(()) => {
                match replace_products(pool, &group.slug, game_products(items, markup_percent))
                    .await
                {
                    Ok(stored) => {
                        report.groups += 1;
                        report.products += stored;
                    }
                    Err(error) => report.errors.push(format!("{provider_name}: {error}")),
                }
            }
            Err(error) => report.errors.push(format!("{provider_name}: {error}")),
        }
    }

    tracing::info!(groups = groups.len(), "game catalog synced");
}

async fn sync_prepaid(
    pool: &SqlitePool,
    provider: &Provider,
    cfg: &Config,
    logos: &LogoCache,
    report: &mut SyncReport,
) {
    let items = match provider.list_prepaid_services().await {
        Ok(items) => items,
        Err(error) => {
            tracing::warn!(%error, "prepaid catalog sync failed");
            report.errors.push(format!("prepaid: {error}"));
            return;
        }
    };

    let mut groups: BTreeMap<(String, String), Vec<PrepaidServiceItem>> = BTreeMap::new();
    for item in items {
        if !is_available(item.status.as_deref()) {
            continue;
        }
        let category = item
            .service_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("lainnya")
            .to_string();
        let brand = brand_key(
            item.brand
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("UMUM"),
        );
        groups.entry((category, brand)).or_default().push(item);
    }

    for ((category, brand), items) in &groups {
        let slug = format!("pp-{}-{}", slugify(category), slugify(brand));
        let markup_percent = cfg.markup_percent_for(category);
        let icon_url = logos.resolve(brand).await;
        if icon_url.is_some() {
            report.logos += 1;
        }
        let group = GroupUpsert {
            icon_url,
            slug,
            name: prepaid_group_name(category, brand),
            provider_name: format!("{category}|{brand}"),
            requires_zone: false,
            user_id_label: prepaid_input_label(category).to_string(),
            zone_label: "Zone ID".to_string(),
            sort_order: prepaid_sort_order(category),
            kind: "prepaid",
            category: Some(category.clone()),
        };

        match upsert_group(pool, &group).await {
            Ok(()) => {
                match replace_products(
                    pool,
                    &group.slug,
                    prepaid_products(items, brand, markup_percent),
                )
                .await
                {
                    Ok(stored) => {
                        report.groups += 1;
                        report.products += stored;
                    }
                    Err(error) => report.errors.push(format!("{}: {error}", group.name)),
                }
            }
            Err(error) => report.errors.push(format!("{}: {error}", group.name)),
        }
    }

    tracing::info!(groups = groups.len(), "prepaid catalog synced");
}

async fn upsert_group(pool: &SqlitePool, group: &GroupUpsert) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO games
            (slug, name, provider_name, icon_url, requires_zone, user_id_label,
             zone_label, sort_order, active, kind, category)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)
         ON CONFLICT(slug) DO UPDATE SET
            name = excluded.name,
            provider_name = excluded.provider_name,
            icon_url = excluded.icon_url,
            requires_zone = excluded.requires_zone,
            user_id_label = excluded.user_id_label,
            zone_label = excluded.zone_label,
            sort_order = excluded.sort_order,
            active = 1,
            kind = excluded.kind,
            category = excluded.category",
    )
    .bind(&group.slug)
    .bind(&group.name)
    .bind(&group.provider_name)
    .bind(&group.icon_url)
    .bind(group.requires_zone as i64)
    .bind(&group.user_id_label)
    .bind(&group.zone_label)
    .bind(group.sort_order)
    .bind(group.kind)
    .bind(&group.category)
    .execute(pool)
    .await?;

    Ok(())
}

async fn replace_products(
    pool: &SqlitePool,
    game_slug: &str,
    products: Vec<NewProduct>,
) -> Result<usize, AppError> {
    let now = crate::now_rfc3339();
    let mut tx = pool.begin().await?;

    sqlx::query("UPDATE products SET available = 0, updated_at = ? WHERE game_slug = ?")
        .bind(&now)
        .bind(game_slug)
        .execute(&mut *tx)
        .await?;

    let stored = products.len();

    for product in products {
        sqlx::query(
            "INSERT INTO products
                (game_slug, code, name, brand, base_price, sell_price,
                 provider_status, available, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(game_slug, code) DO UPDATE SET
                name = excluded.name,
                brand = excluded.brand,
                base_price = excluded.base_price,
                sell_price = excluded.sell_price,
                provider_status = excluded.provider_status,
                available = excluded.available,
                updated_at = excluded.updated_at",
        )
        .bind(game_slug)
        .bind(&product.code)
        .bind(&product.name)
        .bind(&product.brand)
        .bind(product.base_price)
        .bind(product.sell_price)
        .bind(&product.provider_status)
        .bind(product.available as i64)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(stored)
}

fn game_products(items: &[GameServiceItem], markup_percent: i64) -> Vec<NewProduct> {
    items
        .iter()
        .filter_map(|item| {
            let code = item.code.as_deref()?.trim();
            if code.is_empty() {
                return None;
            }
            let base_price = item.price.as_ref().and_then(provider::parse_price)?;
            if base_price <= 0 {
                return None;
            }
            let provider_status = item.status.clone();

            Some(NewProduct {
                code: code.to_string(),
                name: item.name.clone().unwrap_or_else(|| code.to_string()),
                brand: None,
                base_price,
                sell_price: provider::markup(base_price, markup_percent),
                available: is_available(provider_status.as_deref()),
                provider_status,
            })
        })
        .collect()
}

fn prepaid_products(
    items: &[PrepaidServiceItem],
    brand: &str,
    markup_percent: i64,
) -> Vec<NewProduct> {
    items
        .iter()
        .filter_map(|item| {
            let code = item.code.as_deref()?.trim();
            if code.is_empty() {
                return None;
            }
            let base_price = item.price.as_ref().and_then(provider::parse_price)?;
            if base_price <= 0 {
                return None;
            }
            let provider_status = item.status.clone();

            Some(NewProduct {
                code: code.to_string(),
                name: item.name.clone().unwrap_or_else(|| code.to_string()),
                brand: Some(brand.to_string()),
                base_price,
                sell_price: provider::markup(base_price, markup_percent),
                available: is_available(provider_status.as_deref()),
                provider_status,
            })
        })
        .collect()
}

fn is_available(status: Option<&str>) -> bool {
    match status {
        Some(value) => value.eq_ignore_ascii_case("available"),
        None => true,
    }
}

/// Provider price-list items that are not game top-ups (streaming, apps,
/// vouchers) are grouped separately in the storefront.
fn looks_like_app_or_voucher(name: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "voucher",
        "gift card",
        "wallet",
        "google play",
        "garena shell",
        "megaxus",
        "premium",
        "premier",
        "netflix",
        "viu",
        "wetv",
        "iqiyi",
        "vidio",
        "vision plus",
        "bstation",
        "amazon prime",
        "youtube",
        "canva",
        "capcut",
        "alight motion",
        "getcontact",
        "picsart",
        "chatgpt",
        "gemini",
        "perplexity",
        "warp",
        "telegram",
        "razer gold",
        "likee",
        "zepeto",
        "lita",
    ];

    let name = name.to_ascii_lowercase();
    KEYWORDS.iter().any(|keyword| name.contains(keyword))
}

pub fn category_label(category: &str) -> String {
    match category {
        "pulsa-reguler" => "Pulsa Reguler".to_string(),
        "pulsa-transfer" => "Pulsa Transfer".to_string(),
        "pulsa-internasional" => "Pulsa Internasional".to_string(),
        "paket-internet" => "Paket Internet".to_string(),
        "paket-telepon" => "Paket Telepon".to_string(),
        "paket-lainnya" => "Paket Lainnya".to_string(),
        "saldo-emoney" => "Saldo E-Wallet".to_string(),
        "token-pln" => "Token PLN".to_string(),
        "streaming-tv" => "TV & Streaming".to_string(),
        "voucher-game" => "Voucher".to_string(),
        other => title_case(&other.replace('-', " ")),
    }
}

/// Several provider codes point to the same operator (`SMART` and `SMARTFREN`).
fn brand_key(brand: &str) -> String {
    let upper = brand.to_ascii_uppercase();
    match upper.as_str() {
        "SMART" => "SMARTFREN".to_string(),
        other => other.to_string(),
    }
}

fn brand_label(brand: &str) -> String {
    match brand.to_ascii_uppercase().as_str() {
        "GO PAY" => "GoPay".to_string(),
        "SHOPEE PAY" => "ShopeePay".to_string(),
        "BY.U" => "by.U".to_string(),
        "SMART" | "SMARTFREN" => "Smartfren".to_string(),
        "TELKOMSEL" => "Telkomsel".to_string(),
        "INDOSAT" => "Indosat".to_string(),
        "XL" => "XL".to_string(),
        "AXIS" => "Axis".to_string(),
        "TRI" => "Tri".to_string(),
        "PLN" => "PLN".to_string(),
        "DANA" => "DANA".to_string(),
        "OVO" => "OVO".to_string(),
        "LINKAJA" => "LinkAja".to_string(),
        "XOX" => "XOX".to_string(),
        "STARHUB" => "StarHub".to_string(),
        "K-VISION DAN GOL" => "K-Vision & Gol".to_string(),
        "NEX PARABOLA" => "Nex Parabola".to_string(),
        "ORANGE TV" => "Orange TV".to_string(),
        "LIKEE" => "Likee".to_string(),
        "INDOMARET" => "Indomaret".to_string(),
        "ALFAMART VOUCHER" => "Alfamart Voucher".to_string(),
        "RAZER GOLD" => "Razer Gold".to_string(),
        other => title_case(other),
    }
}

fn prepaid_group_name(category: &str, brand: &str) -> String {
    let category_name = category_label(category);
    let upper_brand = brand.to_ascii_uppercase();

    match (category, upper_brand.as_str()) {
        ("token-pln", _) => category_name,
        ("paket-lainnya", "ALFAMART VOUCHER" | "INDOMARET" | "RAZER GOLD") => brand_label(brand),
        _ => format!("{} {}", brand_label(brand), category_name),
    }
}

fn prepaid_input_label(category: &str) -> &'static str {
    match category {
        "token-pln" => "ID Pelanggan / No. Meter",
        "streaming-tv" => "Nomor Pelanggan",
        "voucher-game" => "User ID",
        "pulsa-reguler"
        | "pulsa-transfer"
        | "pulsa-internasional"
        | "paket-internet"
        | "paket-telepon"
        | "saldo-emoney" => "Nomor HP",
        _ => "Nomor Tujuan",
    }
}

fn prepaid_sort_order(category: &str) -> i64 {
    let rank = match category {
        "pulsa-reguler" => 1,
        "pulsa-transfer" => 2,
        "paket-internet" => 3,
        "paket-telepon" => 4,
        "pulsa-internasional" => 5,
        "paket-lainnya" => 6,
        "saldo-emoney" => 7,
        "token-pln" => 8,
        "streaming-tv" => 9,
        "voucher-game" => 10,
        _ => 11,
    };
    1000 + rank * 10
}

fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_dash = true;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "group".to_string()
    } else {
        slug.to_string()
    }
}

fn title_case(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut capitalize = true;

    for ch in value.chars() {
        if ch.is_whitespace() || ch == '-' || ch == '/' {
            capitalize = true;
            out.push(ch);
        } else if capitalize {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.extend(ch.to_lowercase());
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_slugs_from_provider_names() {
        assert_eq!(slugify("Mobile Legends B"), "mobile-legends-b");
        assert_eq!(slugify("PUBG Mobile (ID)"), "pubg-mobile-id");
        assert_eq!(slugify("Honkai: Star Rail"), "honkai-star-rail");
        assert_eq!(slugify("  "), "group");
    }

    #[test]
    fn classifies_apps_and_vouchers() {
        assert!(looks_like_app_or_voucher("Netflix Premium"));
        assert!(looks_like_app_or_voucher("Voucher PSN"));
        assert!(looks_like_app_or_voucher("Steam Wallet Code"));
        assert!(looks_like_app_or_voucher("Canva Pro"));
        assert!(!looks_like_app_or_voucher("Mobile Legends B"));
        assert!(!looks_like_app_or_voucher("Free Fire Max"));
        assert!(!looks_like_app_or_voucher("Delta Force (Steam)"));
    }

    #[test]
    fn labels_prepaid_categories_and_brands() {
        assert_eq!(category_label("token-pln"), "Token PLN");
        assert_eq!(brand_label("GO PAY"), "GoPay");
        assert_eq!(brand_label("XL"), "XL");
        assert_eq!(brand_label("DANA"), "DANA");
        assert_eq!(brand_key("SMART"), "SMARTFREN");
        assert_eq!(brand_key("TRI"), "TRI");
        assert_eq!(brand_label("K-VISION DAN GOL"), "K-Vision & Gol");
        assert_eq!(prepaid_group_name("token-pln", "PLN"), "Token PLN");
        assert_eq!(
            prepaid_group_name("pulsa-reguler", "TELKOMSEL"),
            "Telkomsel Pulsa Reguler"
        );
        assert_eq!(prepaid_input_label("token-pln"), "ID Pelanggan / No. Meter");
        assert!(prepaid_sort_order("token-pln") > prepaid_sort_order("pulsa-reguler"));
    }
}
