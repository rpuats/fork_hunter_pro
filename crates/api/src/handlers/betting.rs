//! Betting handlers - API endpoints for bet placement and execution

use auto_betting::betting::{
    BetInstruction, BetMode, BetResult, BetStatus,
    item_factory, OperatorQueue, QueueItem,
};
use auto_betting::{
    ExecutionOrchestrator, ExecutionState, ForkStatus, BetMode as ExecutionMode,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

use crate::AppState;

/// Execution state response
#[derive(Debug, Serialize)]
pub struct ExecutionStateResponse {
    pub mode: String,
    pub is_running: bool,
    pub is_paused: bool,
    pub active_forks: usize,
    pub pending_confirmations: usize,
    pub total_bets_today: usize,
    pub profit_today: Decimal,
    pub bankroll: Decimal,
    pub max_stake: Decimal,
    pub current_stake: Decimal,
}

/// Set mode request
#[derive(Debug, Deserialize)]
pub struct SetModeRequest {
    pub mode: String, // "auto", "semi", "manual"
}

/// Place bet request
#[derive(Debug, Deserialize)]
pub struct PlaceBetRequest {
    pub fork_id: Uuid,
    pub bookmaker_id: String,
    pub event_name: String,
    pub market: String,
    pub selection: String,
    pub odds: Decimal,
    pub stake: Decimal,
}

/// Confirm bet request
#[derive(Debug, Deserialize)]
pub struct ConfirmBetRequest {
    pub adjusted_stake: Option<Decimal>,
}

/// Get execution state
pub async fn get_execution_state(
    State(state): State<Arc<AppState>>,
) -> Json<ExecutionStateResponse> {
    let orchestrator = state.execution_orchestrator.lock().await;
    let exec_state = orchestrator.get_state();
    
    Json(ExecutionStateResponse {
        mode: match orchestrator.get_mode() {
            BetMode::Auto => "auto".to_string(),
            BetMode::SemiAuto => "semi".to_string(),
            BetMode::Manual => "manual".to_string(),
        },
        is_running: false, // TODO: Get from runner
        is_paused: false,
        active_forks: exec_state.active_forks.len(),
        pending_confirmations: exec_state.account_readiness.values().filter(|r| r.can_place_bets).count(),
        total_bets_today: exec_state.daily_stats.total_bets,
        profit_today: exec_state.daily_stats.profit,
        bankroll: exec_state.bankroll_allocation.total_bankroll,
        max_stake: exec_state.global_limits.max_stake_per_bet,
        current_stake: Decimal::from(1000), // TODO: Calculate current
    })
}

/// Set execution mode
pub async fn set_execution_mode(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SetModeRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let mode = match request.mode.as_str() {
        "auto" => BetMode::Auto,
        "semi" => BetMode::SemiAuto,
        "manual" => BetMode::Manual,
        _ => return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid mode. Use: auto, semi, manual".to_string(),
        )),
    };

    let mut orchestrator = state.execution_orchestrator.lock().await;
    orchestrator.set_mode(mode);

    Ok(Json(serde_json::json!({
        "success": true,
        "mode": request.mode,
    })))
}

/// Start execution
pub async fn start_execution(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: Start betting runner
    Json(serde_json::json!({
        "success": true,
        "message": "Execution started",
    }))
}

/// Stop execution
pub async fn stop_execution(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: Stop betting runner
    Json(serde_json::json!({
        "success": true,
        "message": "Execution stopped",
    }))
}

/// Pause execution
pub async fn pause_execution(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": true,
        "message": "Execution paused",
    }))
}

/// Resume execution
pub async fn resume_execution(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": true,
        "message": "Execution resumed",
    }))
}

/// Place bet
pub async fn place_bet(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PlaceBetRequest>,
) -> Result<Json<BetResult>, (axum::http::StatusCode, String)> {
    let mode = {
        let engine = state.betting_engine.lock().await;
        engine.get_mode()
    };

    let instruction = BetInstruction::new(
        request.fork_id,
        request.bookmaker_id,
        request.event_name,
        request.market,
        request.selection,
        request.odds,
        request.stake,
        mode,
    );

    // Add to engine
    {
        let mut engine = state.betting_engine.lock().await;
        engine.submit_bet(instruction.clone());
    }

    // TODO: Actually execute bet based on mode
    let result = BetResult {
        bet_id: instruction.id.clone(),
        status: BetStatus::Pending,
        external_bet_id: None,
        actual_odds: None,
        error: None,
        screenshot: None,
        placed_at: None,
    };

    Ok(Json(result))
}

/// Confirm bet (for semi-auto mode)
pub async fn confirm_bet(
    State(state): State<Arc<AppState>>,
    Path(bet_id): Path<String>,
    Json(request): Json<ConfirmBetRequest>,
) -> Result<Json<BetResult>, (axum::http::StatusCode, String)> {
    // TODO: Send confirmation to semi-auto runner
    
    let result = BetResult {
        bet_id: bet_id.clone(),
        status: BetStatus::Placed,
        external_bet_id: Some(format!("ext_{}", bet_id)),
        actual_odds: None,
        error: None,
        screenshot: None,
        placed_at: Some(Utc::now()),
    };

    Ok(Json(result))
}

/// Reject bet
pub async fn reject_bet(
    State(state): State<Arc<AppState>>,
    Path(bet_id): Path<String>,
) -> Result<Json<BetResult>, (axum::http::StatusCode, String)> {
    // TODO: Send rejection to semi-auto runner
    
    let result = BetResult {
        bet_id: bet_id.clone(),
        status: BetStatus::Rejected,
        external_bet_id: None,
        actual_odds: None,
        error: Some("Rejected by operator".to_string()),
        screenshot: None,
        placed_at: None,
    };

    Ok(Json(result))
}

/// Get operator queue
pub async fn get_operator_queue(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<QueueItem>> {
    let queue = state.operator_queue.lock().await;
    let items: Vec<QueueItem> = queue.items().iter().cloned().collect();
    Json(items)
}

/// Get current queue item
pub async fn get_current_queue_item(
    State(state): State<Arc<AppState>>,
) -> Json<Option<QueueItem>> {
    let queue = state.operator_queue.lock().await;
    Json(queue.current().cloned())
}

/// Resolve queue item
#[derive(Debug, Deserialize)]
pub struct ResolveQueueItemRequest {
    pub action: String, // "confirm", "reject"
    pub data: Option<serde_json::Value>,
}

pub async fn resolve_queue_item(
    State(state): State<Arc<AppState>>,
    Path(item_id): Path<String>,
    Json(request): Json<ResolveQueueItemRequest>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    let mut queue = state.operator_queue.lock().await;
    
    let item = queue.remove(&item_id)
        .ok_or_else(|| (axum::http::StatusCode::NOT_FOUND, "Item not found".to_string()))?;

    // Handle resolution based on item type and action
    match request.action.as_str() {
        "confirm" => {
            // TODO: Send confirmation to appropriate handler
        }
        "reject" => {
            // TODO: Send rejection
        }
        _ => return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Invalid action. Use: confirm, reject".to_string(),
        )),
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "item_id": item_id,
        "action": request.action,
    })))
}

/// Get pending bets
pub async fn get_pending_bets(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<BetInstruction>> {
    let engine = state.betting_engine.lock().await;
    let bets: Vec<BetInstruction> = engine.get_pending_bets()
        .iter()
        .map(|&b| b.clone())
        .collect();
    Json(bets)
}

/// Betting routes
pub fn betting_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/execution/state", get(get_execution_state))
        .route("/execution/mode", post(set_execution_mode))
        .route("/execution/start", post(start_execution))
        .route("/execution/stop", post(stop_execution))
        .route("/execution/pause", post(pause_execution))
        .route("/execution/resume", post(resume_execution))
        .route("/bet/place", post(place_bet))
        .route("/bet/confirm/:bet_id", post(confirm_bet))
        .route("/bet/reject/:bet_id", post(reject_bet))
        .route("/bet/pending", get(get_pending_bets))
        .route("/operator/queue", get(get_operator_queue))
        .route("/operator/queue/current", get(get_current_queue_item))
        .route("/operator/queue/:item_id/resolve", post(resolve_queue_item))
}
