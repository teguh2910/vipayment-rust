use crate::AppState;
use axum::{
    extract::{DefaultBodyLimit, Request},
    http::header,
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use tower_http::{limit::RequestBodyLimitLayer, services::ServeDir, trace::TraceLayer};
use vipayment_shared::internal;
use vipayment_shared::AppError;

pub fn router(state: AppState) -> Router {
    let internal_auth = state.internal.clone();

    let internal_routes = Router::new()
        .route("/orders/claim", post(crate::internal::claim))
        .route("/orders/pollable", get(crate::internal::pollable))
        .route(
            "/orders/{invoice}/submitted",
            post(crate::internal::submitted),
        )
        .route(
            "/orders/{invoice}/rejected",
            post(crate::internal::rejected),
        )
        .route("/orders/{invoice}/failed", post(crate::internal::failed))
        .route(
            "/orders/{invoice}/status",
            post(crate::internal::apply_status),
        )
        .route_layer(middleware::from_fn(move |request: Request, next: Next| {
            let auth = internal_auth.clone();
            async move { internal::require_internal(auth, request, next).await }
        }));

    let admin = Router::new()
        .route("/payments", get(crate::routes::admin_payments))
        .route(
            "/payments/{invoice}/approve",
            post(crate::routes::approve_payment),
        )
        .route(
            "/payments/{invoice}/reject",
            post(crate::routes::reject_payment),
        )
        .route("/orders", get(crate::routes::admin_orders))
        .route("/orders/{invoice}/retry", post(crate::routes::retry_order))
        .route(
            "/orders/{invoice}/refunded",
            post(crate::routes::mark_refunded),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    let api = Router::new()
        .route("/health", get(crate::routes::health))
        .route("/orders", post(crate::routes::create_order))
        .route("/orders/lookup", get(crate::routes::lookup_orders))
        .route("/orders/{invoice}", get(crate::routes::order_detail))
        .route(
            "/orders/{invoice}/payment-proof",
            post(crate::routes::upload_proof),
        )
        .nest("/admin", admin);

    Router::new()
        .nest("/api/v1", api)
        .nest("/internal", internal_routes)
        .nest_service("/uploads", ServeDir::new(state.cfg.upload_dir.clone()))
        .layer(RequestBodyLimitLayer::new(3 * 1024 * 1024))
        .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn require_admin(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();

    if !vipayment_shared::constant_time_eq(provided, &state.cfg.admin_token) {
        return Err(AppError::Unauthorized);
    }

    Ok(next.run(request).await)
}
