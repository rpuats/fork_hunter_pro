use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::handlers::AppState;
use crate::handlers::*;
use crate::ws::ws_handler;

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_router = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/scanner/status", get(get_scanner_status))
        .route("/api/v1/autobet/status", get(get_autobet_status))
        .route("/api/v1/execution/overview", get(get_execution_overview))
        .route("/api/v1/autobet/start", post(start_autobet))
        .route("/api/v1/autobet/stop", post(stop_autobet))
        .route(
            "/api/v1/autobet/emergency-stop",
            post(emergency_stop_autobet),
        )
        .route("/api/v1/autobet/history", get(get_autobet_history))
        .route("/api/v1/autobet/dry-run", post(autobet_dry_run))
        .route("/api/v1/bankroll", get(get_bankroll))
        .route(
            "/api/v1/bankroll/recommendations",
            get(get_bankroll_recommendations),
        )
        .route("/api/v1/bonuses", get(get_bonuses))
        .route("/api/v1/surebets", get(get_surebets))
        .route("/api/v1/freebets", get(get_freebets))
        .route("/api/v1/freebets/summary", get(get_freebet_summary))
        .route("/api/v1/freebets/plans", get(get_freebet_plans))
        .route("/api/v1/freebets/lifecycle", get(get_freebet_lifecycle))
        .route("/api/v1/value-bets", get(get_value_bets))
        .route("/api/v1/analytics/generosity", get(get_generosity))
        .route("/api/v1/history/stats", get(get_history_stats))
        .route("/api/v1/corridors", get(get_corridors))
        .route("/api/v1/express-forks", get(get_express_forks))
        .route("/api/v1/bookmakers", get(get_bookmakers))
        .route("/api/v1/parsers/coverage", get(get_parsers_coverage))
        .route("/api/v1/parsers/health", get(get_parsers_health))
        .route("/api/v1/accounts", get(get_accounts))
        .route("/api/v1/accounts/summary", get(get_accounts_summary))
        .route("/api/v1/accounts/:bookmaker", get(get_account_by_bookmaker))
        .route(
            "/api/v1/accounts/:bookmaker/balance",
            get(get_account_balance),
        )
        .route(
            "/api/v1/accounts/:bookmaker/refresh",
            post(refresh_account_balance),
        )
        .route(
            "/api/v1/accounts/:bookmaker/control",
            post(update_account_control),
        )
        .route("/api/v1/stakes/validate", post(validate_stake))
        .route("/api/v1/capabilities", get(get_capabilities))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::Extension(state.event_bus.clone()))
        .with_state(state);

    // Serve static files from static/ directory — FALLBACK for non-API routes
    let static_router = Router::new().nest_service(
        "/",
        ServeDir::new("static").not_found_service(ServeDir::new("static/index.html")),
    );

    // API first, static as fallback
    Router::new()
        .merge(api_router)
        .fallback_service(static_router)
}
