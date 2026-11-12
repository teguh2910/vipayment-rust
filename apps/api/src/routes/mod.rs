pub mod admin;
pub mod public;

use crate::{error::AppError, AppState};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderValue, Method},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::CorsLayer, limit::RequestBodyLimitLayer, services::ServeDir, trace::TraceLayer,
};

pub fn router(state: AppState) -> Router {
    let admin = Router::new()
        .route("/payments", get(admin::payments))
        .route("/payments/{invoice}/approve", post(admin::approve_payment))
        .route("/payments/{invoice}/reject", post(admin::reject_payment))
        .route("/orders", get(admin::orders))
        .route("/orders/{invoice}/retry", post(admin::retry_order))
        .route("/orders/{invoice}/refunded", post(admin::mark_refunded))
        .route("/sync-products", post(admin::sync_products))
        .route("/profile", get(admin::profile))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin));

    let api = Router::new()
        .route("/health", get(public::health))
        .route("/games", get(public::games))
        .route("/games/{slug}/products", get(public::products))
        .route("/orders", post(public::create_order))
        .route("/orders/{invoice}", get(public::order_detail))
        .route(
            "/orders/{invoice}/payment-proof",
            post(public::upload_proof),
        )
        .nest("/admin", admin);

    let cors = CorsLayer::new()
        .allow_origin(
            HeaderValue::from_str(&state.cfg.web_origin)
                .unwrap_or_else(|_| HeaderValue::from_static("http://localhost:3000")),
        )
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    let uploads = ServeDir::new(state.cfg.upload_dir.clone());
    let logos = ServeDir::new(state.cfg.logo_dir.clone());

    Router::new()
        .nest("/api/v1", api)
        .nest_service("/uploads", uploads)
        .nest_service("/logos", logos)
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(3 * 1024 * 1024))
        .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn require_admin(
    State(state): State<AppState>,
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

    if !crate::constant_time_eq(provided, &state.cfg.admin_token) {
        return Err(AppError::Unauthorized);
    }

    Ok(next.run(request).await)
}
