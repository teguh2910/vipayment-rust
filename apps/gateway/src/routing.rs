use serde_json::Value;

/// Which backend owns a request path. Kept as a pure function so the routing
/// table can be unit tested without starting a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Catalog,
    Order,
}

/// Routes a path (without the gateway prefix) to the owning service.
///
/// `/games` and `/games/{slug}/products` describe the catalog, everything else
/// under `/api/v1` is order traffic. Admin paths are split by resource.
pub fn backend_for(path: &str) -> Backend {
    if let Some(rest) = path.strip_prefix("/admin/") {
        return if rest.starts_with("sync-products") || rest.starts_with("profile") {
            Backend::Catalog
        } else {
            Backend::Order
        };
    }

    if path == "/games" || (path.starts_with("/games/") && path.ends_with("/products")) {
        return Backend::Catalog;
    }

    Backend::Order
}

/// Merges the catalog and order health payloads into the single document the
/// storefront and README expect. Missing upstreams degrade to `null` fields
/// rather than failing the whole health check.
pub fn merge_health(catalog: Option<Value>, order: Option<Value>) -> Value {
    let mut merged = serde_json::Map::new();
    merged.insert("ok".to_string(), Value::Bool(true));

    for source in [catalog.as_ref(), order.as_ref()] {
        if let Some(Value::Object(fields)) = source {
            for (key, value) in fields {
                if key == "ok" {
                    continue;
                }
                merged.insert(key.clone(), value.clone());
            }
        }
    }

    merged.insert(
        "catalog_service_ok".to_string(),
        Value::Bool(catalog.is_some()),
    );
    merged.insert("order_service_ok".to_string(), Value::Bool(order.is_some()));

    Value::Object(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn routes_catalog_paths_to_the_catalog_service() {
        assert_eq!(backend_for("/games"), Backend::Catalog);
        assert_eq!(
            backend_for("/games/mobile-legends/products"),
            Backend::Catalog
        );
        assert_eq!(backend_for("/admin/sync-products"), Backend::Catalog);
        assert_eq!(backend_for("/admin/profile"), Backend::Catalog);
    }

    #[test]
    fn routes_order_paths_to_the_order_service() {
        assert_eq!(backend_for("/orders"), Backend::Order);
        assert_eq!(backend_for("/orders/lookup"), Backend::Order);
        assert_eq!(backend_for("/orders/INV-1"), Backend::Order);
        assert_eq!(backend_for("/orders/INV-1/payment-proof"), Backend::Order);
        assert_eq!(backend_for("/admin/payments"), Backend::Order);
        assert_eq!(backend_for("/admin/orders"), Backend::Order);
        assert_eq!(backend_for("/admin/orders/INV-1/retry"), Backend::Order);
    }

    #[test]
    fn merges_both_health_documents() {
        let merged = merge_health(
            Some(json!({
                "ok": true,
                "last_sync_at": "2026-01-01T00:00:00Z",
                "logos_cached": 56
            })),
            Some(json!({
                "ok": true,
                "payment_provider": "manual"
            })),
        );

        assert_eq!(merged["ok"], true);
        assert_eq!(merged["logos_cached"], 56);
        assert_eq!(merged["payment_provider"], "manual");
        assert_eq!(merged["catalog_service_ok"], true);
        assert_eq!(merged["order_service_ok"], true);
    }

    #[test]
    fn health_survives_a_degraded_upstream() {
        let merged = merge_health(None, Some(json!({ "payment_provider": "manual" })));

        assert_eq!(merged["ok"], true);
        assert_eq!(merged["catalog_service_ok"], false);
        assert_eq!(merged["order_service_ok"], true);
        assert_eq!(merged["payment_provider"], "manual");
    }
}
