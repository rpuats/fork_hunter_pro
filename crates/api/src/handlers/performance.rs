//! Performance monitoring handlers

use auto_betting::{
    PerformanceMonitor, PerformanceTargets, PerformanceHealth, OperationMetrics,
    get_global_monitor,
};
use axum::{
    Json, Router,
    extract::State,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

/// Performance metrics response
#[derive(Debug, Serialize)]
pub struct PerformanceMetricsResponse {
    pub metrics: Vec<OperationMetrics>,
    pub health: String,
    pub violations: Vec<String>,
    pub targets: PerformanceTargetsResponse,
}

#[derive(Debug, Serialize)]
pub struct PerformanceTargetsResponse {
    pub scan_cycle_ms: u64,
    pub fork_to_display_ms: u64,
    pub auto_bet_ms: u64,
    pub semi_auto_bet_ms: u64,
    pub ui_fps: u32,
}

/// Get performance metrics
pub async fn get_performance_metrics(
    State(_state): State<Arc<AppState>>,
) -> Json<PerformanceMetricsResponse> {
    let monitor = get_global_monitor();

    let (metrics, health, violations) = if let Some(m) = monitor {
        let mets = m.get_metrics().await;
        let report = m.check_health().await;
        (
            mets,
            match report.health {
                PerformanceHealth::Healthy => "healthy",
                PerformanceHealth::Degraded => "degraded",
                PerformanceHealth::Critical => "critical",
            }.to_string(),
            report.violations,
        )
    } else {
        (vec![], "unknown".to_string(), vec![])
    };

    Json(PerformanceMetricsResponse {
        metrics,
        health,
        violations,
        targets: PerformanceTargetsResponse {
            scan_cycle_ms: 500,
            fork_to_display_ms: 1000,
            auto_bet_ms: 5000,
            semi_auto_bet_ms: 10000,
            ui_fps: 60,
        },
    })
}

/// Reset performance metrics
pub async fn reset_performance_metrics(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    if let Some(monitor) = get_global_monitor() {
        monitor.reset().await;
    }

    Json(serde_json::json!({
        "success": true,
        "message": "Performance metrics reset",
    }))
}

/// Performance routes
pub fn performance_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/performance", get(get_performance_metrics))
        .route("/api/v1/performance/reset", get(reset_performance_metrics))
}
