use axum::extract::{State, Query};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use engine::freebet::FreebetHunter;
use engine::generosity::GenerosityIndexCalc;
use persistence::history::SurebetHistory;
use scanner::ScannerRunner;
use shared::models::{FreebetOpportunity, GenerosityIndex, ScannerMetrics, Surebet, ValueBet};
use shared::{CorridorOpportunity, ExpressFork};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone, Default)]
pub struct SurebetsQuery {
    pub limit: Option<i32>,
}

#[derive(Serialize)]
pub struct CapabilityItem {
    pub id: &'static str,
    pub area: &'static str,
    pub status: &'static str,
    pub current_surface: Vec<&'static str>,
    pub planned_surface: Vec<&'static str>,
    pub backing_crates: Vec<&'static str>,
    pub notes: &'static str,
}

#[derive(Serialize)]
pub struct DesktopUiField {
    pub key: &'static str,
    pub source: &'static str,
    pub required: bool,
    pub notes: &'static str,
}

#[derive(Serialize)]
pub struct ApiSurfacePlan {
    pub parser_coverage: Vec<serde_json::Value>,
    pub capabilities: Vec<CapabilityItem>,
    pub desktop_ui_fields: Vec<DesktopUiField>,
}
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub scanner: Arc<ScannerRunner>,
    pub history: Arc<SurebetHistory>,
    pub freebet_hunter: Arc<FreebetHunter>,
    pub generosity_index: Arc<GenerosityIndexCalc>,
    pub event_bus: Arc<shared::EventBus>,
}

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now().to_rfc3339(),
        }
    }

    pub fn error(msg: &str) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.to_string()),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

pub async fn health_check() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::ok(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

pub async fn get_metrics(State(state): State<AppState>) -> Json<ApiResponse<ScannerMetrics>> {
    // Try to get real metrics from scanner
    let metrics = state.scanner.get_metrics();
    match metrics {
        Some(m) => Json(ApiResponse::ok(m)),
        None => Json(ApiResponse::ok(ScannerMetrics {
            cycle_time_ms: 0,
            events_parsed: 0,
            surebets_found: 0,
            active_bookmakers: 7,
            failed_bookmakers: 0,
            cache_hit_rate: 0.0,
            memory_mb: 0.0,
            timestamp: Utc::now(),
        })),
    }
}

pub async fn get_scanner_status(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let scanner_state = state.scanner.get_state();
    let active_parsers = scanner_state
        .last_metrics
        .as_ref()
        .map(|metrics| metrics.active_bookmakers)
        .unwrap_or(0);

    Json(ApiResponse::ok(serde_json::json!({
        "running": scanner_state.running,
        "cycle_count": scanner_state.cycle_count,
        "active_parsers": active_parsers,
        "last_metrics": scanner_state.last_metrics,
    })))
}

pub async fn get_surebets(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Result<Json<ApiResponse<Vec<Surebet>>>, StatusCode> {
    let limit = params.limit.unwrap_or(50) as usize;
    // Читаем из кэша сканнера вместо SQLite
    let surebets = state.scanner.get_surebets(limit);
    Ok(Json(ApiResponse::ok(surebets)))
}

pub async fn get_freebets(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<FreebetOpportunity>>> {
    // Получаем opportunities из FreebetHunter
    // В текущей реализации FreebetHunter имеет метод scan который ищет фрибет-вилки
    // Пока возвращаем пустой список — реальная интеграция требует запуска сканера
    let opportunities = state.freebet_hunter.scan_freebets();
    Json(ApiResponse::ok(opportunities))
}

pub async fn get_value_bets(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Json<ApiResponse<Vec<ValueBet>>> {
    let limit = params.limit.unwrap_or(50) as usize;
    // Value bets вычисляются на лету из текущего состояния сканнера
    let value_bets = state.scanner.get_value_bets(limit);
    Json(ApiResponse::ok(value_bets))
}

pub async fn get_generosity(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<GenerosityIndex>>> {
    let indices = state.generosity_index.get_all_indices(shared::Sport::Football);
    Json(ApiResponse::ok(indices))
}

pub async fn get_history_stats(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    match state.history.get_stats().await {
        Ok(stats) => Ok(Json(ApiResponse::ok(serde_json::json!({
            "total": stats.total,
            "avg_profit": stats.avg_profit,
            "max_profit": stats.max_profit,
            "total_stake": stats.total_stake,
        })))),
        Err(e) => {
            tracing::error!(error = e.to_string(), "Failed to get stats");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_corridors(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<CorridorOpportunity>>> {
    // Get corridors from scanner
    let corridors = state.scanner.get_corridors(100);
    Json(ApiResponse::ok(corridors))
}

pub async fn get_express_forks(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ExpressFork>>> {
    // Get express forks from scanner
    let forks = state.scanner.get_express_forks(100);
    Json(ApiResponse::ok(forks))
}

pub async fn get_bookmakers(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    // Return list of active bookmakers with stats
    let _metrics = state.scanner.get_metrics();

    let bookmakers = vec![
        serde_json::json!({ "name": "Pari", "slug": "pari", "status": "active", "events": 6608 }),
        serde_json::json!({ "name": "Fonbet", "slug": "fonbet", "status": "active", "events": 6826 }),
        serde_json::json!({ "name": "Bettery", "slug": "bettery", "status": "active", "events": 6843 }),
        serde_json::json!({ "name": "Marathon", "slug": "marathon", "status": "active", "events": 6566 }),
        serde_json::json!({ "name": "24bet", "slug": "bet24", "status": "active", "events": 6557 }),
        serde_json::json!({ "name": "Leon", "slug": "leon", "status": "active", "events": 3676 }),
        serde_json::json!({ "name": "Sportbet", "slug": "sportbet", "status": "active", "events": 258 }),
    ];
    
    Json(ApiResponse::ok(bookmakers))
}

pub async fn get_capabilities() -> Json<ApiResponse<ApiSurfacePlan>> {
    let parser_coverage = vec![
        serde_json::json!({"slug": "pari", "status": "active", "parser_type": "api", "source": "crates/parsers/src/pari.rs", "notes": "Registered in ParserFactory and exposed via /api/v1/bookmakers."}),
        serde_json::json!({"slug": "fonbet", "status": "active", "parser_type": "api", "source": "crates/parsers/src/fonbet.rs", "notes": "Registered in ParserFactory."}),
        serde_json::json!({"slug": "bettery", "status": "active", "parser_type": "api", "source": "crates/parsers/src/bettery.rs", "notes": "Registered in ParserFactory."}),
        serde_json::json!({"slug": "marathon", "status": "active", "parser_type": "api", "source": "crates/parsers/src/marathon.rs", "notes": "Registered in ParserFactory."}),
        serde_json::json!({"slug": "leon", "status": "active", "parser_type": "api", "source": "crates/parsers/src/leon.rs", "notes": "Registered in ParserFactory."}),
        serde_json::json!({"slug": "sportbet", "status": "active", "parser_type": "api", "source": "crates/parsers/src/sportbet.rs", "notes": "Registered in ParserFactory, lower observed event volume in current placeholder bookmaker stats."}),
        serde_json::json!({"slug": "bet24", "status": "in_progress", "parser_type": "api", "source": "crates/parsers/src/bet24.rs", "notes": "Module is newly added and registered in ParserFactory but still untracked in git status during this audit."}),
        serde_json::json!({"slug": "olimp", "status": "blocked", "parser_type": "api", "source": "crates/parsers/src/olimp.rs", "notes": "Implementation exists, but ParserFactory keeps it disabled because competition payload structure is not normalized yet."}),
        serde_json::json!({"slug": "winline", "status": "not_ported", "parser_type": "legacy", "source": "legacy/python only", "notes": "Referenced in config defaults/tests, but not registered in current Rust ParserFactory."}),
        serde_json::json!({"slug": "betcity", "status": "not_ported", "parser_type": "legacy", "source": "legacy/python only", "notes": "Module export exists, but no active registration in current Rust ParserFactory."}),
        serde_json::json!({"slug": "zenit", "status": "not_ported", "parser_type": "legacy", "source": "legacy/python only", "notes": "Module export exists, but no active registration in current Rust ParserFactory."}),
        serde_json::json!({"slug": "baltbet", "status": "not_ported", "parser_type": "legacy", "source": "legacy/python only", "notes": "Module export exists, but no active registration in current Rust ParserFactory."})
    ];

    let capabilities = vec![
        CapabilityItem {
            id: "parser-coverage",
            area: "scanner",
            status: "partial",
            current_surface: vec!["GET /api/v1/bookmakers", "GET /api/v1/scanner/status", "GET /api/v1/metrics"],
            planned_surface: vec!["GET /api/v1/capabilities", "GET /api/v1/parsers/coverage", "GET /api/v1/parsers/health"],
            backing_crates: vec!["crates/parsers", "crates/scanner", "crates/api"],
            notes: "Current API only exposes coarse bookmaker placeholders. Desktop UI needs per-parser status, parser type, health, and last-seen volume.",
        },
        CapabilityItem {
            id: "autobetting-controls",
            area: "execution",
            status: "backend-only",
            current_surface: vec!["AutoBetEngine::start", "AutoBetEngine::stop", "AutoBetEngine::emergency_stop", "AutoBetEngine::get_status", "AutoBetEngine::get_limiter_stats"],
            planned_surface: vec!["GET /api/v1/autobet/status", "POST /api/v1/autobet/start", "POST /api/v1/autobet/stop", "POST /api/v1/autobet/emergency-stop", "GET /api/v1/autobet/history?limit="],
            backing_crates: vec!["crates/auto_betting", "crates/scanner", "crates/api"],
            notes: "Engine exists and is wired into GhostScanner, but AppState does not currently expose it and no HTTP/bot control plane exists.",
        },
        CapabilityItem {
            id: "freebet-planning",
            area: "bonus",
            status: "partial",
            current_surface: vec!["GET /api/v1/freebets", "BonusHunter::get_best_bonuses", "BonusHunter::create_bonus_plan", "BonusHunter::get_bonus_plan"],
            planned_surface: vec!["GET /api/v1/bonuses", "POST /api/v1/bonuses/plans", "GET /api/v1/bonuses/plans/:bookmaker", "PATCH /api/v1/bonuses/plans/:bookmaker/progress"],
            backing_crates: vec!["crates/bonus_hunter", "crates/engine", "crates/api"],
            notes: "Freebet API currently returns scan output only. Bonus planner logic is available in Rust but unreachable from API/UI.",
        },
        CapabilityItem {
            id: "bankroll-deposit-guidance",
            area: "risk",
            status: "backend-only",
            current_surface: vec!["BankrollManager::get_state", "BankrollManager::calculate_optimal_stake", "BankrollManager::get_rebalance_recommendations"],
            planned_surface: vec!["GET /api/v1/bankroll", "POST /api/v1/bankroll/balances", "GET /api/v1/bankroll/rebalance", "POST /api/v1/bankroll/stake-advice"],
            backing_crates: vec!["crates/bankroll_manager", "crates/api"],
            notes: "Data model already includes recommended_deposit/recommended_withdraw, so API can expose this without inventing new business logic.",
        },
        CapabilityItem {
            id: "stake-min-max-checks",
            area: "validation",
            status: "partial",
            current_surface: vec!["BetLimiter::can_bet", "AutoBetEngine::place_surebet"],
            planned_surface: vec!["POST /api/v1/stakes/validate", "GET /api/v1/autobet/limits"],
            backing_crates: vec!["crates/auto_betting", "crates/bankroll_manager", "crates/api"],
            notes: "Only global hourly/daily limits and profit thresholds exist today. There is no bookmaker-level min/max stake validation contract yet.",
        },
        CapabilityItem {
            id: "desktop-ui-feed",
            area: "desktop-ui",
            status: "needs-contract",
            current_surface: vec!["GET /api/v1/surebets", "GET /api/v1/freebets", "GET /api/v1/corridors", "GET /api/v1/express-forks", "GET /api/v1/history/stats", "GET /api/v1/bookmakers", "GET /ws"],
            planned_surface: vec!["GET /api/v1/capabilities", "GET /api/v1/autobet/status", "GET /api/v1/bankroll", "GET /api/v1/parsers/coverage", "GET /api/v1/bonuses"],
            backing_crates: vec!["crates/api", "desktop-ui"],
            notes: "The UI can render list views now, but not operator controls, planner progress, or bankroll recommendations.",
        },
        CapabilityItem {
            id: "telegram-bot-ops",
            area: "bot",
            status: "minimal",
            current_surface: vec!["/start", "/status", "/help", "TelegramBot::notify_surebet", "TelegramBot::notify_system"],
            planned_surface: vec!["/autobet_status", "/autobet_start", "/autobet_stop", "/bankroll", "/bonus_plan <bookmaker>"],
            backing_crates: vec!["crates/bot", "crates/api", "crates/auto_betting", "crates/bankroll_manager", "crates/bonus_hunter"],
            notes: "Bot currently provides notifications and basic alive/status checks only.",
        },
    ];

    let desktop_ui_fields = vec![
        DesktopUiField { key: "surebet.id", source: "/api/v1/surebets", required: true, notes: "Stable row key and action target." },
        DesktopUiField { key: "surebet.legs[].url", source: "/api/v1/surebets", required: true, notes: "Needed for deep-link/open-bookmaker actions." },
        DesktopUiField { key: "parser.status / parser.type / parser.last_error", source: "/api/v1/parsers/coverage", required: true, notes: "Needed for diagnostics panel and parser filter chips." },
        DesktopUiField { key: "autobet.running / emergency_stopped / limits", source: "/api/v1/autobet/status", required: true, notes: "Needed for topbar safety controls." },
        DesktopUiField { key: "bankroll.bookmakers[].recommended_deposit / recommended_withdraw", source: "/api/v1/bankroll", required: true, notes: "Needed for cash allocation widgets." },
        DesktopUiField { key: "bonus.plan.progress_percent / next_step", source: "/api/v1/bonuses/plans/:bookmaker", required: true, notes: "Needed for freebet/bonus execution workflow." },
        DesktopUiField { key: "stake_validation.accepted / reason / suggested_stake", source: "/api/v1/stakes/validate", required: false, notes: "Needed before enabling one-click execution." },
    ];

    Json(ApiResponse::ok(ApiSurfacePlan {
        parser_coverage,
        capabilities,
        desktop_ui_fields,
    }))
}
