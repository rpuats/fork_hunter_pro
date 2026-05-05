use axum::http::StatusCode;
use axum::routing::{any, delete, get, post};
use axum::{Json, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::handlers::*;
use crate::handlers::{
    telegram_history, telegram_status, telegram_update_config, ApiResponse, AppState,
};
use crate::handlers::auth::{
    add_account, authenticate_all, authenticate_one, get_balance, list_accounts, logout,
    remove_account, submit_2fa, submit_captcha, supported_bookmakers,
};
use crate::handlers::betting::{
    confirm_bet, get_execution_state, get_operator_queue, get_pending_bets, pause_execution,
    place_bet, reject_bet, resolve_queue_item, resume_execution, set_execution_mode,
    start_execution, stop_execution, get_current_queue_item,
};
use crate::handlers::performance::{
    get_performance_metrics, reset_performance_metrics,
};
use crate::ws::{ws_handler, ws_surebets_v1_handler};
use crate::ws_execution::ws_execution_handler;

async fn api_not_found() -> (StatusCode, Json<ApiResponse<serde_json::Value>>) {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::error("API route not found")),
    )
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_router = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/scanner/status", get(get_scanner_status))
        .route("/api/v1/autobet/status", get(get_autobet_status))
        .route("/api/v1/execution/overview", get(get_execution_overview))
        .route("/api/v1/execution/ledger", get(get_execution_ledger))
        .route("/api/v1/execution/state", get(get_execution_state))
        .route(
            "/api/v1/execution/operator-queue",
            get(get_execution_operator_queue),
        )
        .route(
            "/api/v1/execution/semi-auto-queue",
            get(get_semi_auto_queue),
        )
        .route(
            "/api/v1/execution/semi-auto-queue/:coupon_id/confirm",
            post(confirm_semi_auto_coupon),
        )
        .route("/api/v1/autobet/start", post(start_autobet))
        .route("/api/v1/autobet/stop", post(stop_autobet))
        .route(
            "/api/v1/autobet/emergency-stop",
            post(emergency_stop_autobet),
        )
        .route("/api/v1/autobet/history", get(get_autobet_history))
        .route("/api/v1/autobet/dry-run", post(autobet_dry_run))
        .route("/api/v1/autobet/execute-leg", post(autobet_execute_leg))
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
        .route("/api/v1/middles", get(get_middles))
        .route("/api/v1/odds-errors", get(get_odds_errors))
        .route("/api/v1/analytics/generosity", get(get_generosity))
        .route("/api/v1/history", get(get_history))
        .route("/api/v1/history/stats", get(get_history_stats))
        .route("/api/v1/corridors", get(get_corridors))
        .route("/api/v1/express-forks", get(get_express_forks))
        .route("/api/v1/bookmakers", get(get_bookmakers))
        .route(
            "/api/v1/bookmakers/status-catalog",
            get(get_bookmaker_status_catalog),
        )
        .route("/api/v1/parsers/coverage", get(get_parsers_coverage))
        .route("/api/v1/parsers/health", get(get_parsers_health))
        .route(
            "/api/v1/parsers/promotion-kpi",
            get(get_parsers_promotion_kpi),
        )
        .route("/api/v1/swarm/status", get(get_swarm_status))
        .route("/api/v1/accounts", get(get_accounts))
        .route(
            "/api/v1/accounts/bootstrap-session",
            post(bootstrap_account_session),
        )
        .route(
            "/api/v1/accounts/import-session",
            post(bootstrap_account_session),
        )
        .route(
            "/api/v1/accounts/automated-login",
            post(automated_account_login),
        )
        .route(
            "/api/v2/accounts/bulk-automated-login",
            post(bulk_automated_login),
        )
        .route(
            "/api/v1/accounts/test-account",
            post(bootstrap_account_session),
        )
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
        // New auth module routes
        .route("/api/v1/auth/accounts", get(list_accounts))
        .route("/api/v1/auth/accounts", post(add_account))
        .route("/api/v1/auth/accounts/:bk_id", delete(remove_account))
        .route("/api/v1/auth/authenticate-all", post(authenticate_all))
        .route("/api/v1/auth/authenticate/:bk_id", post(authenticate_one))
        .route("/api/v1/auth/logout/:bk_id", post(logout))
        .route("/api/v1/auth/captcha/:bk_id", post(submit_captcha))
        .route("/api/v1/auth/2fa/:bk_id", post(submit_2fa))
        .route("/api/v1/auth/balance/:bk_id", get(get_balance))
        .route("/api/v1/auth/supported", get(supported_bookmakers))
        // Betting/Execution routes
        .route("/api/v1/execution/state", get(get_execution_state))
        .route("/api/v1/execution/mode", post(set_execution_mode))
        .route("/api/v1/execution/start", post(start_execution))
        .route("/api/v1/execution/stop", post(stop_execution))
        .route("/api/v1/execution/pause", post(pause_execution))
        .route("/api/v1/execution/resume", post(resume_execution))
        .route("/api/v1/bet/place", post(place_bet))
        .route("/api/v1/bet/confirm/:bet_id", post(confirm_bet))
        .route("/api/v1/bet/reject/:bet_id", post(reject_bet))
        .route("/api/v1/bet/pending", get(get_pending_bets))
        .route("/api/v1/operator/queue", get(get_operator_queue))
        .route("/api/v1/operator/queue/current", get(get_current_queue_item))
        .route("/api/v1/operator/queue/:item_id/resolve", post(resolve_queue_item))
        // Performance monitoring
        .route("/api/v1/performance", get(get_performance_metrics))
        .route("/api/v1/performance/reset", get(reset_performance_metrics))
        .route("/api/v1/stakes/validate", post(validate_stake))
        .route("/api/v1/capabilities", get(get_capabilities))
        .route("/api/v1/telegram/status", get(telegram_status))
        .route("/api/v1/telegram/config", post(telegram_update_config))
        .route("/api/v1/telegram/history", get(telegram_history))
        .route("/api/v2/surebets", get(get_surebets))
        .route("/api/v2/opportunities", get(get_opportunities_v2))
        .route("/api/v2/surebets/:id/execute", post(execute_surebet_v2))
        .route("/api/v2/middles", get(get_middles))
        .route("/api/v2/valuebets", get(get_value_bets))
        .route("/api/v2/bonuses", get(get_bonuses))
        .route("/api/v2/bonuses/calendar", get(get_bonus_calendar_v2))
        .route("/api/v2/bankroll/allocate", post(post_bankroll_allocate_v2))
        .route("/api/v2/bankroll/advice", get(get_bankroll_advice_v2))
        .route("/api/v2/freebets/lifecycle", get(get_freebet_lifecycle))
        .route(
            "/api/v2/freebets/funding-advice",
            get(get_freebet_funding_advice_v2),
        )
        .route("/api/v2/freebets/qualify", post(post_freebet_qualify_v2))
        .route("/api/v2/accounts", get(get_accounts))
        .route(
            "/api/v2/accounts/bootstrap-session",
            post(bootstrap_account_session),
        )
        .route(
            "/api/v2/accounts/import-session",
            post(bootstrap_account_session),
        )
        .route(
            "/api/v2/accounts/automated-login",
            post(automated_account_login),
        )
        .route(
            "/api/v2/accounts/test-account",
            post(bootstrap_account_session),
        )
        .route("/api/v2/accounts/readiness", get(get_accounts_readiness_v2))
        .route("/api/v2/accounts/:bookmaker", get(get_account_by_bookmaker))
        .route(
            "/api/v2/accounts/:bookmaker/refresh",
            post(refresh_account_balance),
        )
        .route("/api/v2/analytics/pll", get(get_history_stats))
        .route("/api/v2/analytics/clv", get(get_clv_analytics_v2))
        .route("/api/v2/execution/queue", get(get_execution_operator_queue))
        .route(
            "/api/v2/execution/semi-auto-queue",
            get(get_semi_auto_queue),
        )
        .route(
            "/api/v2/execution/semi-auto-queue/:coupon_id/confirm",
            post(confirm_semi_auto_coupon),
        )
        .route("/api/v2/execution/execute-leg", post(autobet_execute_leg))
        .route("/api/v2/execution/panic", post(post_execution_panic_v2))
        .route("/api/v2/health/ghost", get(get_ghost_health_v2))
        .route("/ws", get(ws_handler))
        .route("/ws/v1/surebets", get(ws_surebets_v1_handler))
        .route("/ws/execution", get(ws_execution_handler))
        .route("/api", any(api_not_found))
        .route("/api/*path", any(api_not_found))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::Extension(state.event_bus.clone()))
        .with_state(state);

    // Serve static files from static/ directory — FALLBACK for non-API routes
    let static_router = Router::new().nest_service(
        "/",
        ServeDir::new("static").not_found_service(ServeFile::new("static/index.html")),
    );

    // API first, static as fallback
    Router::new()
        .merge(api_router)
        .fallback_service(static_router)
}
