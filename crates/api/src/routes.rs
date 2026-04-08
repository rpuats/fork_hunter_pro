use axum::routing::get;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;

use crate::handlers::*;
use crate::ws::ws_handler;
use crate::handlers::AppState;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_router = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/scanner/status", get(get_scanner_status))
        .route("/api/v1/surebets", get(get_surebets))
        .route("/api/v1/freebets", get(get_freebets))
        .route("/api/v1/analytics/generosity", get(get_generosity))
        .route("/api/v1/history/stats", get(get_history_stats))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::Extension(state.event_bus.clone()))
        .with_state(state);

    // Serve static files from static/ directory — FALLBACK for non-API routes
    let static_router = Router::new()
        .nest_service("/", ServeDir::new("static").not_found_service(ServeDir::new("static/index.html")));

    // API first, static as fallback
    Router::new()
        .merge(api_router)
        .fallback_service(static_router)
}
