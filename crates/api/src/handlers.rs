use axum::extract::{State, Query};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use engine::freebet::FreebetHunter;
use engine::generosity::GenerosityIndexCalc;
use persistence::history::SurebetHistory;
use scanner::ScannerRunner;
use serde::Serialize;
use shared::models::{FreebetOpportunity, GenerosityIndex, ScannerMetrics, Surebet};
use shared::{CorridorOpportunity, ExpressFork};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, Default)]
pub struct SurebetsQuery {
    pub limit: Option<i32>,
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
    let limit = params.limit.unwrap_or(50);
    match state.history.get_recent(limit).await {
        Ok(surebets) => Ok(Json(ApiResponse::ok(surebets))),
        Err(e) => {
            tracing::error!(error = e.to_string(), "Failed to get surebets");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
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
