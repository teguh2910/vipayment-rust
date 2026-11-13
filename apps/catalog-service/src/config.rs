use std::collections::HashMap;
use std::net::SocketAddr;
use vipayment_shared::config::{bind_addr, parse_num, var, var_or, ProviderConfig};

#[derive(Clone)]
pub struct CatalogConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub provider: ProviderConfig,
    pub markup_percent: i64,
    pub markup_by_category: HashMap<String, i64>,
    pub markup_fixed_by_category: HashMap<String, i64>,
    pub catalog_categories: Option<Vec<String>>,
    pub logo_dir: String,
    pub games_config_path: String,
    pub logos_config_path: String,
}

impl Default for CatalogConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8081".parse().expect("valid default bind addr"),
            database_url: "sqlite://data/catalog.db".to_string(),
            provider: ProviderConfig::default(),
            markup_percent: 5,
            markup_by_category: HashMap::new(),
            markup_fixed_by_category: HashMap::new(),
            catalog_categories: None,
            logo_dir: "data/logos".to_string(),
            games_config_path: "config/games.json".to_string(),
            logos_config_path: "config/logos.json".to_string(),
        }
    }
}

impl CatalogConfig {
    pub fn from_env() -> Result<Self, String> {
        let defaults = Self::default();

        let markup_by_category = match var("MARKUP_BY_CATEGORY") {
            Some(raw) => parse_markup_map(&raw)?,
            None => HashMap::new(),
        };

        let markup_fixed_by_category = match var("MARKUP_FIXED_BY_CATEGORY") {
            Some(raw) => parse_fixed_markup_map(&raw)?,
            None => HashMap::new(),
        };

        Ok(Self {
            bind_addr: bind_addr("BIND_ADDR", "127.0.0.1:8081")?,
            database_url: var_or("DATABASE_URL", &defaults.database_url),
            provider: ProviderConfig::from_env()?,
            markup_percent: parse_num("MARKUP_PERCENT", defaults.markup_percent)?,
            markup_by_category,
            markup_fixed_by_category,
            catalog_categories: parse_catalog_categories(var("CATALOG_CATEGORIES").as_deref())?,
            logo_dir: var_or("LOGO_DIR", &defaults.logo_dir),
            games_config_path: var_or("GAMES_CONFIG_PATH", &defaults.games_config_path),
            logos_config_path: var_or("LOGOS_CONFIG_PATH", &defaults.logos_config_path),
        })
    }

    /// Whether a catalog category may be sold in the storefront. When
    /// `CATALOG_CATEGORIES` is unset every category is allowed. Game/app
    /// groups have no category, so setting an allowlist hides them.
    pub fn category_allowed(&self, category: Option<&str>) -> bool {
        let Some(allowed) = &self.catalog_categories else {
            return true;
        };

        category
            .map(|value| {
                allowed
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(value.trim()))
            })
            .unwrap_or(false)
    }

    /// Pricing rule for a catalog category. A fixed rupiah margin takes
    /// precedence over a percentage; both fall back to `MARKUP_PERCENT`.
    pub fn category_pricing(&self, category: &str) -> CategoryPricing {
        let key = category.to_ascii_lowercase();

        if let Some(amount) = self.markup_fixed_by_category.get(&key) {
            return CategoryPricing::Fixed(*amount);
        }

        CategoryPricing::Percent(
            self.markup_by_category
                .get(&key)
                .copied()
                .unwrap_or(self.markup_percent),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryPricing {
    Percent(i64),
    Fixed(i64),
}

impl CategoryPricing {
    pub fn sell_price(self, base_price: i64) -> i64 {
        match self {
            Self::Percent(percent) => vipayment_shared::provider::markup(base_price, percent),
            Self::Fixed(amount) => base_price + amount,
        }
    }
}

fn parse_markup_map(raw: &str) -> Result<HashMap<String, i64>, String> {
    parse_category_map(raw, "MARKUP_BY_CATEGORY", 1000)
}

fn parse_fixed_markup_map(raw: &str) -> Result<HashMap<String, i64>, String> {
    parse_category_map(raw, "MARKUP_FIXED_BY_CATEGORY", 1_000_000)
}

/// Parses the storefront category allowlist (`CATALOG_CATEGORIES`), a
/// comma-separated list of category slugs. Unset means no restriction.
fn parse_catalog_categories(raw: Option<&str>) -> Result<Option<Vec<String>>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let categories: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .collect();

    if categories.is_empty() {
        return Err("CATALOG_CATEGORIES tidak boleh kosong".to_string());
    }

    Ok(Some(categories))
}

fn parse_category_map(raw: &str, name: &str, max: i64) -> Result<HashMap<String, i64>, String> {
    let parsed: HashMap<String, i64> =
        serde_json::from_str(raw).map_err(|e| format!("{name} bukan JSON yang valid: {e}"))?;

    for (category, value) in &parsed {
        if !(0..=max).contains(value) {
            return Err(format!("{name}[{category}] harus antara 0 dan {max}"));
        }
    }

    Ok(parsed
        .into_iter()
        .map(|(category, value)| (category.to_ascii_lowercase(), value))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_category_markups_case_insensitively() {
        let markups = parse_markup_map(r#"{"game": 7, "APP": 10, "pulsa-reguler": 4}"#).unwrap();

        assert_eq!(markups.get("game"), Some(&7));
        assert_eq!(markups.get("app"), Some(&10));
        assert_eq!(markups.get("pulsa-reguler"), Some(&4));
    }

    #[test]
    fn rejects_invalid_category_markups() {
        assert!(parse_markup_map("not json").is_err());
        assert!(parse_markup_map(r#"{"game": -1}"#).is_err());
        assert!(parse_markup_map(r#"{"game": 1001}"#).is_err());
    }

    #[test]
    fn parses_fixed_category_markups() {
        let markups =
            parse_fixed_markup_map(r#"{"pulsa-reguler": 500, "TOKEN-PLN": 1000}"#).unwrap();

        assert_eq!(markups.get("pulsa-reguler"), Some(&500));
        assert_eq!(markups.get("token-pln"), Some(&1000));
        assert!(parse_fixed_markup_map(r#"{"token-pln": -1}"#).is_err());
        assert!(parse_fixed_markup_map(r#"{"token-pln": 1000001}"#).is_err());
    }

    #[test]
    fn falls_back_to_default_markup() {
        let config = CatalogConfig::default();
        assert_eq!(
            config.category_pricing("game"),
            CategoryPricing::Percent(config.markup_percent)
        );
        assert_eq!(
            config.category_pricing("unknown"),
            CategoryPricing::Percent(config.markup_percent)
        );
    }

    #[test]
    fn fixed_margin_takes_precedence_over_percent() {
        let mut config = CatalogConfig::default();
        config.markup_by_category.insert("token-pln".to_string(), 4);
        config
            .markup_fixed_by_category
            .insert("token-pln".to_string(), 1000);

        assert_eq!(
            config.category_pricing("token-pln"),
            CategoryPricing::Fixed(1000)
        );
        assert_eq!(CategoryPricing::Fixed(1000).sell_price(10_000), 11_000);
        assert_eq!(CategoryPricing::Percent(5).sell_price(10_000), 10_500);
    }

    #[test]
    fn parses_catalog_category_allowlist() {
        assert_eq!(parse_catalog_categories(None).unwrap(), None);
        assert_eq!(
            parse_catalog_categories(Some(" Token-PLN , pulsa-reguler ,")).unwrap(),
            Some(vec!["token-pln".to_string(), "pulsa-reguler".to_string()])
        );
        assert!(parse_catalog_categories(Some(" , ")).is_err());
    }

    #[test]
    fn catalog_allowlist_gates_categories() {
        let mut config = CatalogConfig::default();
        assert!(config.category_allowed(None));
        assert!(config.category_allowed(Some("game")));

        config.catalog_categories = Some(vec!["token-pln".to_string()]);
        assert!(config.category_allowed(Some("token-pln")));
        assert!(config.category_allowed(Some(" TOKEN-PLN ")));
        assert!(!config.category_allowed(Some("game")));
        assert!(!config.category_allowed(None));
    }
}
