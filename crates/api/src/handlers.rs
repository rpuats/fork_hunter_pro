use auto_betting::engine::AutoBetEngine;
use auto_betting::limiter::BetLimiterStats;
use auto_betting::validator::StakeValidator;
use auto_betting::ExecutionRegistry;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use bankroll_manager::manager::BankrollManager;
use bonus_hunter::hunter::BonusHunter;
use chrono::Utc;
use engine::freebet::FreebetHunter;
use engine::generosity::GenerosityIndexCalc;
use persistence::history::SurebetHistory;
use scanner::ScannerRunner;
use serde::{Deserialize, Serialize};
use shared::models::{
    AccountSessionSummary, AutoBetDryRunLegRequest, AutoBetDryRunLegResponse, AutoBetDryRunRequest,
    AutoBetDryRunResponse, AutoBetStatus, BankrollState, BetExecutionRequest, BetPlacement,
    BetStatus, BonusInfo, BookmakerAccount, BookmakerBalance, BookmakerBalanceRefresh,
    BookmakerBalanceSnapshot, BookmakerExecutionCapability, BookmakerExecutionMode,
    BookmakerMetadata, BookmakerSession, DepositAllocationGuidance, ExecutionOverview,
    ExecutionPlacementSummary, FreebetBookmakerAllocation, FreebetConversionPlan,
    FreebetLifecycleStage, FreebetLifecycleState, FreebetLifecycleSummary, FreebetOpportunity,
    FreebetPlanRequest, FreebetProgressStatus, FreebetRolloverProgress, GenerosityIndex,
    ParserCoverage, ParserHealth, ScannerMetrics, StakeValidationDecision,
    StakeValidationPreflightRequest, StakeValidationPreflightResponse, StakeValidationRequest,
    Surebet, ValueBet,
};
use shared::{CorridorOpportunity, ExpressFork};
use std::collections::HashMap;

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
    pub parser_coverage: Vec<ParserCoverage>,
    pub capabilities: Vec<CapabilityItem>,
    pub desktop_ui_fields: Vec<DesktopUiField>,
}
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub scanner: Arc<ScannerRunner>,
    pub bookmakers: Arc<Vec<BookmakerMetadata>>,
    pub parser_coverage: Arc<Vec<ParserCoverage>>,
    pub parser_health: Arc<Vec<ParserHealth>>,
    pub history: Arc<SurebetHistory>,
    pub freebet_hunter: Arc<FreebetHunter>,
    pub generosity_index: Arc<GenerosityIndexCalc>,
    pub auto_bet_engine: Arc<AutoBetEngine>,
    pub bankroll_manager: Arc<BankrollManager>,
    pub bonus_hunter: Arc<BonusHunter>,
    pub event_bus: Arc<shared::EventBus>,
}

#[derive(Serialize)]
pub struct AutoBetStatusResponse {
    pub status: AutoBetStatus,
    pub limits: BetLimiterStats,
}

#[derive(Serialize)]
pub struct BankrollRecommendationsResponse {
    pub rebalance: Vec<BookmakerBalance>,
    pub deposit_guidance: DepositAllocationGuidance,
}

#[derive(Debug, Serialize)]
pub struct AccountStateResponse {
    pub bookmaker: String,
    pub capability: BookmakerExecutionCapability,
    pub account: Option<BookmakerAccount>,
    pub session: Option<BookmakerSession>,
    pub balance: Option<BookmakerBalanceSnapshot>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountControlUpdateRequest {
    pub enabled: Option<bool>,
    pub armed: Option<bool>,
    pub confirm_dry_run_only: bool,
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

fn execution_registry(state: &AppState) -> Arc<ExecutionRegistry> {
    state.auto_bet_engine.execution_registry()
}

fn build_recommended_freebet_plan(
    bonus_hunter: &BonusHunter,
    opportunity: &FreebetOpportunity,
) -> FreebetConversionPlan {
    bonus_hunter.build_freebet_plan(&FreebetPlanRequest {
        freebet_bookmaker: opportunity.bookmaker.clone(),
        qualifying_bookmaker: opportunity.bookmaker.clone(),
        hedge_bookmaker: opportunity.hedge_bookmaker.clone(),
        market: opportunity.market.clone(),
        qualifying_selection: opportunity.selection.clone(),
        freebet_selection: opportunity.selection.clone(),
        hedge_selection: opportunity.hedge_selection.clone(),
        freebet_amount: opportunity.freebet_amount,
        qualifying_odds: opportunity.back_odds,
        back_odds: opportunity.back_odds,
        lay_odds: opportunity.lay_odds,
        estimated_qualifying_loss: 0.0,
        exchange_like_hedge: false,
    })
}

fn build_rollover_progress(
    bonus: &BonusInfo,
    bonus_plan: Option<&shared::BonusPlan>,
) -> Option<FreebetRolloverProgress> {
    let required_turnover = bonus_plan
        .map(|plan| plan.wager_required)
        .unwrap_or_else(|| bonus.amount * bonus.wager_requirement)
        .max(0.0);
    if required_turnover <= 0.0 {
        return None;
    }

    let completed_turnover = bonus_plan
        .map(|plan| plan.wager_done)
        .unwrap_or(bonus.wager_progress)
        .clamp(0.0, required_turnover);
    let remaining_turnover = (required_turnover - completed_turnover).max(0.0);
    let progress_percent = if required_turnover > 0.0 {
        ((completed_turnover / required_turnover) * 100.0).min(100.0)
    } else {
        100.0
    };

    let status = if completed_turnover <= 0.0 {
        FreebetProgressStatus::NotStarted
    } else if remaining_turnover <= 0.0 {
        FreebetProgressStatus::Completed
    } else {
        FreebetProgressStatus::InProgress
    };

    Some(FreebetRolloverProgress {
        required_turnover,
        completed_turnover,
        remaining_turnover,
        progress_percent,
        status,
    })
}

fn build_freebet_allocation(
    bookmaker: &str,
    balances: &HashMap<String, BookmakerBalance>,
    deposits: &HashMap<String, shared::DepositAllocationTarget>,
) -> Option<FreebetBookmakerAllocation> {
    let balance = balances.get(bookmaker);
    let deposit = deposits.get(bookmaker);

    if balance.is_none() && deposit.is_none() {
        return None;
    }

    Some(FreebetBookmakerAllocation {
        bookmaker: bookmaker.to_string(),
        available_balance: balance.map(|item| item.available),
        recommended_deposit: deposit.map(|item| item.recommended_deposit),
        deposit_gap: deposit.map(|item| item.deposit_gap),
        urgency: deposit.as_ref().map(|item| item.urgency.clone()),
        note: deposit
            .map(|item| item.note.clone())
            .unwrap_or_else(|| "no extra deposit guidance required".into()),
    })
}

fn infer_freebet_stage(
    opportunity: Option<&FreebetOpportunity>,
    bonus: Option<&BonusInfo>,
    plan: Option<&FreebetConversionPlan>,
    rollover: Option<&FreebetRolloverProgress>,
) -> FreebetLifecycleStage {
    if rollover
        .map(|item| item.status == FreebetProgressStatus::Completed)
        .unwrap_or(false)
    {
        return FreebetLifecycleStage::RolloverCompleted;
    }

    if rollover
        .map(|item| item.status == FreebetProgressStatus::InProgress)
        .unwrap_or(false)
    {
        return FreebetLifecycleStage::RolloverInProgress;
    }

    if plan.is_some() {
        return FreebetLifecycleStage::Planned;
    }

    if let Some(bonus) = bonus {
        return match bonus.status {
            shared::BonusStatus::Available => FreebetLifecycleStage::Available,
            shared::BonusStatus::Claimed => FreebetLifecycleStage::Qualified,
            shared::BonusStatus::Wagering => FreebetLifecycleStage::RolloverInProgress,
            shared::BonusStatus::Completed => FreebetLifecycleStage::RolloverCompleted,
            shared::BonusStatus::Expired | shared::BonusStatus::Rejected => {
                if opportunity.is_some() {
                    FreebetLifecycleStage::Discovered
                } else {
                    FreebetLifecycleStage::Available
                }
            }
        };
    }

    let _ = opportunity;
    FreebetLifecycleStage::Discovered
}

fn collect_freebet_lifecycle(state: &AppState) -> Vec<FreebetLifecycleState> {
    let opportunities = state.freebet_hunter.scan_freebets();
    let freebet_bonuses = state.bonus_hunter.get_active_freebet_bonuses();
    let bonus_plans = state.bonus_hunter.get_all_bonus_plans();
    let bankroll_state = state.bankroll_manager.get_state();
    let deposit_guidance = state.bankroll_manager.get_deposit_allocation_guidance();

    let mut best_opportunities: HashMap<String, FreebetOpportunity> = HashMap::new();
    for opportunity in opportunities {
        best_opportunities
            .entry(opportunity.bookmaker.clone())
            .and_modify(|current| {
                if opportunity.guaranteed_profit > current.guaranteed_profit {
                    *current = opportunity.clone();
                }
            })
            .or_insert(opportunity);
    }

    let bonus_by_bookmaker: HashMap<String, BonusInfo> = freebet_bonuses
        .into_iter()
        .map(|bonus| (bonus.bookmaker.clone(), bonus))
        .collect();
    let bonus_plans_by_bookmaker: HashMap<String, shared::BonusPlan> = bonus_plans
        .into_iter()
        .map(|plan| (plan.bookmaker.clone(), plan))
        .collect();
    let balances_by_bookmaker: HashMap<String, BookmakerBalance> = bankroll_state
        .bookmakers
        .into_iter()
        .map(|balance| (balance.bookmaker.clone(), balance))
        .collect();
    let deposits_by_bookmaker: HashMap<String, shared::DepositAllocationTarget> = deposit_guidance
        .targets
        .into_iter()
        .map(|target| (target.bookmaker.clone(), target))
        .collect();

    let mut bookmakers: Vec<String> = best_opportunities.keys().cloned().collect();
    for bookmaker in bonus_by_bookmaker.keys() {
        if !bookmakers.iter().any(|item| item == bookmaker) {
            bookmakers.push(bookmaker.clone());
        }
    }
    for bookmaker in bonus_plans_by_bookmaker.keys() {
        if !bookmakers.iter().any(|item| item == bookmaker) {
            bookmakers.push(bookmaker.clone());
        }
    }

    let mut states = Vec::new();
    for bookmaker in bookmakers {
        let opportunity = best_opportunities.get(&bookmaker).cloned();
        let bonus = bonus_by_bookmaker.get(&bookmaker).cloned();
        let bonus_plan = bonus_plans_by_bookmaker.get(&bookmaker);
        let plan = opportunity
            .as_ref()
            .map(|item| build_recommended_freebet_plan(&state.bonus_hunter, item));
        let rollover = bonus
            .as_ref()
            .and_then(|item| build_rollover_progress(item, bonus_plan));
        let allocation =
            build_freebet_allocation(&bookmaker, &balances_by_bookmaker, &deposits_by_bookmaker);
        let lifecycle_stage = infer_freebet_stage(
            opportunity.as_ref(),
            bonus.as_ref(),
            plan.as_ref(),
            rollover.as_ref(),
        );

        states.push(FreebetLifecycleState {
            bookmaker,
            lifecycle_stage,
            opportunity,
            bonus,
            plan,
            rollover,
            allocation,
            updated_at: Utc::now(),
        });
    }

    states.sort_by(|a, b| a.bookmaker.cmp(&b.bookmaker));
    states
}

fn get_account_state(
    registry: &ExecutionRegistry,
    bookmaker: &str,
) -> Option<AccountStateResponse> {
    let known_bookmakers = registry.list_bookmakers();
    if !known_bookmakers.iter().any(|item| item == bookmaker) {
        return None;
    }

    Some(AccountStateResponse {
        bookmaker: bookmaker.to_string(),
        capability: registry.get_capability(bookmaker),
        account: registry.get_account(bookmaker),
        session: registry.get_session(bookmaker),
        balance: registry.get_balance_snapshot(bookmaker),
    })
}

fn operator_target_mode(enabled: bool, armed: bool) -> BookmakerExecutionMode {
    if !enabled {
        BookmakerExecutionMode::Disabled
    } else if armed {
        BookmakerExecutionMode::Armed
    } else {
        BookmakerExecutionMode::DryRun
    }
}

fn apply_account_control_update(
    registry: &ExecutionRegistry,
    bookmaker: &str,
    request: AccountControlUpdateRequest,
) -> Result<AccountStateResponse, (StatusCode, String)> {
    if !request.confirm_dry_run_only {
        return Err((
            StatusCode::BAD_REQUEST,
            "account control updates require confirm_dry_run_only=true".into(),
        ));
    }

    let current = registry.get_account(bookmaker).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            "bookmaker account state not found".to_string(),
        )
    })?;

    if request.enabled.is_none() && request.armed.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "at least one of enabled or armed must be provided".into(),
        ));
    }

    let enabled = request.enabled.unwrap_or(current.enabled);
    let armed = request.armed.unwrap_or(current.mode.is_armed());
    let target_mode = operator_target_mode(enabled, armed);

    registry
        .update_account_control_state(bookmaker, enabled, target_mode)
        .map_err(|error| (StatusCode::BAD_REQUEST, error))?;

    get_account_state(registry, bookmaker).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "updated account state could not be loaded".to_string(),
        )
    })
}

fn build_account_session_summary(registry: &ExecutionRegistry) -> AccountSessionSummary {
    let mut summary = AccountSessionSummary {
        total_bookmakers: 0,
        accounts_configured: 0,
        accounts_enabled: 0,
        disabled_accounts: 0,
        sessions_configured: 0,
        sessions_authenticated: 0,
        balances_cached: 0,
        ready_for_execution: 0,
        ready_for_dry_run: 0,
    };

    for bookmaker in registry.list_bookmakers() {
        summary.total_bookmakers += 1;

        let capability = registry.get_capability(&bookmaker);
        let account = registry.get_account(&bookmaker);
        let session = registry.get_session(&bookmaker);
        let balance = registry.get_balance_snapshot(&bookmaker);

        if let Some(account) = account.as_ref() {
            summary.accounts_configured += 1;
            if account.enabled {
                summary.accounts_enabled += 1;
            } else {
                summary.disabled_accounts += 1;
            }
        }

        if session.is_some() {
            summary.sessions_configured += 1;
        }

        if session
            .as_ref()
            .map(|item| matches!(item.state, shared::BookmakerSessionState::Active))
            .unwrap_or(false)
        {
            summary.sessions_authenticated += 1;
        }

        if balance.is_some() {
            summary.balances_cached += 1;
        }

        let executable = account
            .as_ref()
            .map(|item| {
                item.enabled && !matches!(item.mode, shared::BookmakerExecutionMode::Disabled)
            })
            .unwrap_or(false);
        let session_ready = !capability.requires_session
            || session
                .as_ref()
                .map(|item| matches!(item.state, shared::BookmakerSessionState::Active))
                .unwrap_or(false);
        let balance_ready = !capability.supports_balance_snapshot || balance.is_some();

        if executable && session_ready && balance_ready {
            summary.ready_for_dry_run += 1;

            if capability.supports_bet_placement
                && account
                    .as_ref()
                    .map(|item| item.mode.allows_submission_path())
                    .unwrap_or(false)
            {
                summary.ready_for_execution += 1;
            }
        }
    }

    summary
}

fn build_execution_placement_summary(placements: &[BetPlacement]) -> ExecutionPlacementSummary {
    let mut summary = ExecutionPlacementSummary {
        total: placements.len(),
        pending: 0,
        placed: 0,
        settled: 0,
        cancelled: 0,
        errors: 0,
    };

    for placement in placements {
        match placement.status {
            BetStatus::Pending => summary.pending += 1,
            BetStatus::Placed => summary.placed += 1,
            BetStatus::Settled => summary.settled += 1,
            BetStatus::Cancelled => summary.cancelled += 1,
            BetStatus::Error => summary.errors += 1,
        }
    }

    summary
}

fn build_execution_overview(state: &AppState) -> ExecutionOverview {
    let registry = execution_registry(state);
    let placements = state.auto_bet_engine.get_history(100);

    ExecutionOverview {
        autobet_status: state.auto_bet_engine.get_status(),
        accounts: build_account_session_summary(&registry),
        recent_placements: build_execution_placement_summary(&placements),
        generated_at: Utc::now(),
    }
}

fn build_freebet_lifecycle_summary(states: &[FreebetLifecycleState]) -> FreebetLifecycleSummary {
    let mut summary = FreebetLifecycleSummary {
        total_bookmakers: states.len(),
        opportunities: 0,
        active_bonuses: 0,
        tracked_plans: 0,
        deposit_required_bookmakers: 0,
        discovered: 0,
        available: 0,
        qualified: 0,
        planned: 0,
        rollover_in_progress: 0,
        rollover_completed: 0,
        total_freebet_amount: 0.0,
        total_estimated_profit: 0.0,
        generated_at: Utc::now(),
    };

    for state in states {
        if let Some(opportunity) = state.opportunity.as_ref() {
            summary.opportunities += 1;
            summary.total_freebet_amount += opportunity.freebet_amount;
        }

        if state.bonus.is_some() {
            summary.active_bonuses += 1;
        }

        if let Some(plan) = state.plan.as_ref() {
            summary.tracked_plans += 1;
            summary.total_estimated_profit += plan.estimated_profit;
        }

        if state
            .allocation
            .as_ref()
            .and_then(|item| item.recommended_deposit)
            .unwrap_or(0.0)
            > 0.0
        {
            summary.deposit_required_bookmakers += 1;
        }

        match state.lifecycle_stage {
            FreebetLifecycleStage::Discovered => summary.discovered += 1,
            FreebetLifecycleStage::Available => summary.available += 1,
            FreebetLifecycleStage::Qualified => summary.qualified += 1,
            FreebetLifecycleStage::Planned => summary.planned += 1,
            FreebetLifecycleStage::RolloverInProgress => summary.rollover_in_progress += 1,
            FreebetLifecycleStage::RolloverCompleted => summary.rollover_completed += 1,
        }
    }

    summary
}

async fn build_stake_preflight(
    registry: &ExecutionRegistry,
    request: StakeValidationPreflightRequest,
) -> Result<StakeValidationPreflightResponse, String> {
    let bookmaker = request.bookmaker.clone();
    let capability = registry.get_capability(&bookmaker);
    let account = registry.get_account(&bookmaker);
    let balance_refresh = registry.refresh_balance_snapshot(&bookmaker).await?;

    let mut validation = StakeValidator::validate(&StakeValidationRequest {
        bookmaker: bookmaker.clone(),
        desired_stake: request.desired_stake,
        min_stake: request.min_stake,
        max_stake: request.max_stake,
        bookmaker_available_balance: balance_refresh
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.available_balance),
        bankroll_available_balance: request.bankroll_available_balance,
        allow_auto_adjust: request.allow_auto_adjust,
    });

    if account.is_none() {
        validation.decision = StakeValidationDecision::Reject;
        validation
            .reasons
            .push("no enabled bookmaker account is registered".into());
    }

    if let Some(account) = account.as_ref() {
        if !account.enabled {
            validation.decision = StakeValidationDecision::Reject;
            validation
                .reasons
                .push("bookmaker account is disabled".into());
        } else if matches!(account.mode, shared::BookmakerExecutionMode::Disabled) {
            validation.decision = StakeValidationDecision::Reject;
            validation
                .reasons
                .push("bookmaker account mode is disabled".into());
        }
    }

    if capability.requires_session && !balance_refresh.session_status.authenticated {
        validation.decision = StakeValidationDecision::Reject;
        validation.reasons.push(
            balance_refresh
                .session_status
                .detail
                .clone()
                .unwrap_or_else(|| "bookmaker session is not authenticated".into()),
        );
    }

    if capability.supports_balance_snapshot && balance_refresh.snapshot.is_none() {
        validation.decision = StakeValidationDecision::Reject;
        validation.reasons.push(
            balance_refresh
                .detail
                .clone()
                .unwrap_or_else(|| "bookmaker balance snapshot is unavailable".into()),
        );
    }

    let executable = account.as_ref().map(|item| item.enabled).unwrap_or(false)
        && account
            .as_ref()
            .map(|item| !matches!(item.mode, shared::BookmakerExecutionMode::Disabled))
            .unwrap_or(false)
        && (!capability.requires_session || balance_refresh.session_status.authenticated)
        && (!capability.supports_balance_snapshot || balance_refresh.snapshot.is_some())
        && !matches!(validation.decision, StakeValidationDecision::Reject);
    let arm_required = capability.supports_bet_placement;
    let armed_for_execution = executable
        && account
            .as_ref()
            .map(|item| item.mode.is_armed())
            .unwrap_or(false);
    let placement_ready = armed_for_execution
        && account
            .as_ref()
            .map(|item| item.mode.allows_submission_path())
            .unwrap_or(false)
        && capability.supports_bet_placement;
    let real_money_enabled = placement_ready && capability.supports_real_money;
    let dry_run_ready = executable
        && capability.supports_dry_run
        && account
            .as_ref()
            .map(|item| item.mode.allows_dry_run())
            .unwrap_or(false);

    Ok(StakeValidationPreflightResponse {
        bookmaker,
        capability,
        account,
        balance_refresh,
        validation,
        executable,
        dry_run_ready,
        arm_required,
        armed_for_execution,
        placement_ready,
        real_money_enabled,
    })
}

async fn build_dry_run_leg(
    registry: &ExecutionRegistry,
    request: AutoBetDryRunLegRequest,
) -> Result<AutoBetDryRunLegResponse, String> {
    let preflight = build_stake_preflight(
        registry,
        StakeValidationPreflightRequest {
            bookmaker: request.bookmaker.clone(),
            desired_stake: request.desired_stake,
            min_stake: request.min_stake,
            max_stake: request.max_stake,
            bankroll_available_balance: request.bankroll_available_balance,
            allow_auto_adjust: request.allow_auto_adjust,
        },
    )
    .await?;

    let execution_request = BetExecutionRequest {
        bookmaker: request.bookmaker,
        event_id: request.event_id,
        market: request.market,
        selection: request.selection,
        odds: request.odds,
        stake: preflight.validation.adjusted_stake,
        allow_dry_run: true,
        reference: request.reference,
    };

    let receipt = if preflight.dry_run_ready {
        Some(registry.dry_run_bet(&execution_request).await?)
    } else {
        None
    };

    Ok(AutoBetDryRunLegResponse {
        preflight,
        execution_request,
        receipt,
    })
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

pub async fn get_scanner_status(
    State(state): State<AppState>,
) -> Json<ApiResponse<serde_json::Value>> {
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
    let opportunities = state.freebet_hunter.scan_freebets();
    Json(ApiResponse::ok(opportunities))
}

pub async fn get_freebet_plans(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<FreebetConversionPlan>>> {
    let plans = state
        .freebet_hunter
        .scan_freebets()
        .into_iter()
        .map(|opportunity| build_recommended_freebet_plan(&state.bonus_hunter, &opportunity))
        .collect();

    Json(ApiResponse::ok(plans))
}

pub async fn get_freebet_lifecycle(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<FreebetLifecycleState>>> {
    Json(ApiResponse::ok(collect_freebet_lifecycle(&state)))
}

pub async fn get_freebet_summary(
    State(state): State<AppState>,
) -> Json<ApiResponse<FreebetLifecycleSummary>> {
    let lifecycle = collect_freebet_lifecycle(&state);
    Json(ApiResponse::ok(build_freebet_lifecycle_summary(&lifecycle)))
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
    let indices = state
        .generosity_index
        .get_all_indices(shared::Sport::Football);
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

pub async fn get_autobet_status(
    State(state): State<AppState>,
) -> Json<ApiResponse<AutoBetStatusResponse>> {
    Json(ApiResponse::ok(AutoBetStatusResponse {
        status: state.auto_bet_engine.get_status(),
        limits: state.auto_bet_engine.get_limiter_stats(),
    }))
}

pub async fn get_execution_overview(
    State(state): State<AppState>,
) -> Json<ApiResponse<ExecutionOverview>> {
    Json(ApiResponse::ok(build_execution_overview(&state)))
}

pub async fn start_autobet(
    State(state): State<AppState>,
) -> Json<ApiResponse<AutoBetStatusResponse>> {
    state.auto_bet_engine.start();
    Json(ApiResponse::ok(AutoBetStatusResponse {
        status: state.auto_bet_engine.get_status(),
        limits: state.auto_bet_engine.get_limiter_stats(),
    }))
}

pub async fn stop_autobet(
    State(state): State<AppState>,
) -> Json<ApiResponse<AutoBetStatusResponse>> {
    state.auto_bet_engine.stop();
    Json(ApiResponse::ok(AutoBetStatusResponse {
        status: state.auto_bet_engine.get_status(),
        limits: state.auto_bet_engine.get_limiter_stats(),
    }))
}

pub async fn emergency_stop_autobet(
    State(state): State<AppState>,
) -> Json<ApiResponse<AutoBetStatusResponse>> {
    state.auto_bet_engine.emergency_stop();
    Json(ApiResponse::ok(AutoBetStatusResponse {
        status: state.auto_bet_engine.get_status(),
        limits: state.auto_bet_engine.get_limiter_stats(),
    }))
}

pub async fn get_autobet_history(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Json<ApiResponse<Vec<BetPlacement>>> {
    let limit = params.limit.unwrap_or(50) as usize;
    Json(ApiResponse::ok(state.auto_bet_engine.get_history(limit)))
}

pub async fn get_bankroll(State(state): State<AppState>) -> Json<ApiResponse<BankrollState>> {
    Json(ApiResponse::ok(state.bankroll_manager.get_state()))
}

pub async fn get_bankroll_recommendations(
    State(state): State<AppState>,
) -> Json<ApiResponse<BankrollRecommendationsResponse>> {
    Json(ApiResponse::ok(BankrollRecommendationsResponse {
        rebalance: state.bankroll_manager.get_rebalance_recommendations(),
        deposit_guidance: state.bankroll_manager.get_deposit_allocation_guidance(),
    }))
}

pub async fn get_bonuses(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Json<ApiResponse<Vec<BonusInfo>>> {
    let limit = params.limit.unwrap_or(50) as usize;
    Json(ApiResponse::ok(state.bonus_hunter.get_best_bonuses(limit)))
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
) -> Json<ApiResponse<Vec<BookmakerMetadata>>> {
    Json(ApiResponse::ok((*state.bookmakers).clone()))
}

pub async fn get_parsers_coverage(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ParserCoverage>>> {
    Json(ApiResponse::ok((*state.parser_coverage).clone()))
}

pub async fn get_parsers_health(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ParserHealth>>> {
    Json(ApiResponse::ok((*state.parser_health).clone()))
}

pub async fn get_accounts(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<AccountStateResponse>>> {
    let registry = execution_registry(&state);
    let accounts = registry
        .list_bookmakers()
        .into_iter()
        .filter_map(|bookmaker| get_account_state(&registry, &bookmaker))
        .collect();

    Json(ApiResponse::ok(accounts))
}

pub async fn get_accounts_summary(
    State(state): State<AppState>,
) -> Json<ApiResponse<AccountSessionSummary>> {
    let registry = execution_registry(&state);
    Json(ApiResponse::ok(build_account_session_summary(&registry)))
}

pub async fn get_account_by_bookmaker(
    State(state): State<AppState>,
    axum::extract::Path(bookmaker): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    let registry = execution_registry(&state);

    match get_account_state(&registry, &bookmaker) {
        Some(account) => (StatusCode::OK, Json(ApiResponse::ok(account))),
        None => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AccountStateResponse>::error(
                "bookmaker account state not found",
            )),
        ),
    }
}

pub async fn get_account_balance(
    State(state): State<AppState>,
    axum::extract::Path(bookmaker): axum::extract::Path<String>,
) -> axum::response::Response {
    let registry = execution_registry(&state);

    if get_account_state(&registry, &bookmaker).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<Option<BookmakerBalanceSnapshot>>::error(
                "bookmaker account state not found",
            )),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(ApiResponse::ok(registry.get_balance_snapshot(&bookmaker))),
    )
        .into_response()
}

pub async fn refresh_account_balance(
    State(state): State<AppState>,
    axum::extract::Path(bookmaker): axum::extract::Path<String>,
) -> axum::response::Response {
    let registry = execution_registry(&state);

    if get_account_state(&registry, &bookmaker).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<BookmakerBalanceRefresh>::error(
                "bookmaker account state not found",
            )),
        )
            .into_response();
    }

    match registry.refresh_balance_snapshot(&bookmaker).await {
        Ok(snapshot) => (StatusCode::OK, Json(ApiResponse::ok(snapshot))).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<BookmakerBalanceRefresh>::error(&error)),
        )
            .into_response(),
    }
}

pub async fn update_account_control(
    State(state): State<AppState>,
    axum::extract::Path(bookmaker): axum::extract::Path<String>,
    Json(request): Json<AccountControlUpdateRequest>,
) -> axum::response::Response {
    let registry = execution_registry(&state);

    match apply_account_control_update(&registry, &bookmaker, request) {
        Ok(account) => (StatusCode::OK, Json(ApiResponse::ok(account))).into_response(),
        Err((status, error)) => (
            status,
            Json(ApiResponse::<AccountStateResponse>::error(&error)),
        )
            .into_response(),
    }
}

pub async fn validate_stake(
    State(state): State<AppState>,
    Json(request): Json<StakeValidationPreflightRequest>,
) -> axum::response::Response {
    let registry = execution_registry(&state);

    match build_stake_preflight(&registry, request).await {
        Ok(response) => (StatusCode::OK, Json(ApiResponse::ok(response))).into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::<StakeValidationPreflightResponse>::error(
                &error,
            )),
        )
            .into_response(),
    }
}

pub async fn autobet_dry_run(
    State(state): State<AppState>,
    Json(request): Json<AutoBetDryRunRequest>,
) -> axum::response::Response {
    let registry = execution_registry(&state);
    let mut legs = Vec::with_capacity(request.legs.len());

    for leg in request.legs {
        match build_dry_run_leg(&registry, leg).await {
            Ok(response) => legs.push(response),
            Err(error) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(ApiResponse::<AutoBetDryRunResponse>::error(&error)),
                )
                    .into_response();
            }
        }
    }

    let ready_legs = legs.iter().filter(|leg| leg.preflight.executable).count();
    let rejected_legs = legs.len().saturating_sub(ready_legs);

    (
        StatusCode::OK,
        Json(ApiResponse::ok(AutoBetDryRunResponse {
            all_legs_executable: rejected_legs == 0,
            ready_legs,
            rejected_legs,
            legs,
        })),
    )
        .into_response()
}

pub async fn get_capabilities(State(state): State<AppState>) -> Json<ApiResponse<ApiSurfacePlan>> {
    let parser_coverage = (*state.parser_coverage).clone();

    let capabilities = vec![
        CapabilityItem {
            id: "parser-coverage",
            area: "scanner",
            status: "partial",
            current_surface: vec!["GET /api/v1/bookmakers", "GET /api/v1/parsers/coverage", "GET /api/v1/parsers/health", "GET /api/v1/scanner/status", "GET /api/v1/metrics"],
            planned_surface: vec!["GET /api/v1/capabilities"],
            backing_crates: vec!["crates/parsers", "crates/scanner", "crates/api"],
            notes: "Coverage and health endpoints now expose parser type, readiness, and diagnostic checks, but runtime last-seen volume is still coarse.",
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
        DesktopUiField {
            key: "surebet.id",
            source: "/api/v1/surebets",
            required: true,
            notes: "Stable row key and action target.",
        },
        DesktopUiField {
            key: "surebet.legs[].url",
            source: "/api/v1/surebets",
            required: true,
            notes: "Needed for deep-link/open-bookmaker actions.",
        },
        DesktopUiField {
            key: "parser.status / parser.type / parser.last_error",
            source: "/api/v1/parsers/coverage",
            required: true,
            notes: "Needed for diagnostics panel and parser filter chips.",
        },
        DesktopUiField {
            key: "autobet.running / emergency_stopped / limits",
            source: "/api/v1/autobet/status",
            required: true,
            notes: "Needed for topbar safety controls.",
        },
        DesktopUiField {
            key: "bankroll.bookmakers[].recommended_deposit / recommended_withdraw",
            source: "/api/v1/bankroll",
            required: true,
            notes: "Needed for cash allocation widgets.",
        },
        DesktopUiField {
            key: "bonus.plan.progress_percent / next_step",
            source: "/api/v1/bonuses/plans/:bookmaker",
            required: true,
            notes: "Needed for freebet/bonus execution workflow.",
        },
        DesktopUiField {
            key: "stake_validation.accepted / reason / suggested_stake",
            source: "/api/v1/stakes/validate",
            required: false,
            notes: "Needed before enabling one-click execution.",
        },
    ];

    Json(ApiResponse::ok(ApiSurfacePlan {
        parser_coverage,
        capabilities,
        desktop_ui_fields,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use shared::{
        BonusDifficulty, BonusStatus, BonusType, BookmakerAccount, BookmakerBalanceRefreshState,
        BookmakerExecutionMode, BookmakerSession, BookmakerSessionState, Event, Sport,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_freebet_bonus(status: BonusStatus, wager_progress: f64) -> BonusInfo {
        BonusInfo {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            bonus_type: BonusType::Freebet,
            name: "Freebet 1000".into(),
            amount: 1_000.0,
            currency: "RUB".into(),
            wager_requirement: 3.0,
            min_odds: 1.8,
            max_bet: 1_000.0,
            expiry_days: 7,
            real_value: 700.0,
            ev: 650.0,
            difficulty: BonusDifficulty::Medium,
            status,
            wager_progress,
            detected_at: Utc::now(),
            url: None,
        }
    }

    fn make_freebet_opportunity() -> FreebetOpportunity {
        FreebetOpportunity {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            hedge_bookmaker: "fonbet".into(),
            event: Event {
                id: "evt-1".into(),
                sport: Sport::Football,
                league: "Test League".into(),
                home_team: "A".into(),
                away_team: "B".into(),
                start_time: None,
                is_live: false,
                bookmaker_slug: "pari".into(),
                raw_url: None,
                extra: HashMap::new(),
            },
            market: "1X2".into(),
            selection: "1".into(),
            hedge_selection: "1".into(),
            back_odds: 4.2,
            lay_odds: 2.1,
            freebet_amount: 1_000.0,
            guaranteed_profit: 120.0,
            roi: 12.0,
            detected_at: Utc::now(),
        }
    }

    #[test]
    fn freebet_rollover_progress_uses_turnover_values() {
        let progress =
            build_rollover_progress(&make_freebet_bonus(BonusStatus::Wagering, 1_500.0), None)
                .expect("progress should exist");

        assert_eq!(progress.required_turnover, 3_000.0);
        assert_eq!(progress.completed_turnover, 1_500.0);
        assert_eq!(progress.remaining_turnover, 1_500.0);
        assert_eq!(progress.status, FreebetProgressStatus::InProgress);
    }

    #[test]
    fn freebet_lifecycle_prefers_rollover_completion() {
        let rollover = FreebetRolloverProgress {
            required_turnover: 3_000.0,
            completed_turnover: 3_000.0,
            remaining_turnover: 0.0,
            progress_percent: 100.0,
            status: FreebetProgressStatus::Completed,
        };

        let stage = infer_freebet_stage(None, None, None, Some(&rollover));
        assert_eq!(stage, FreebetLifecycleStage::RolloverCompleted);
    }

    #[test]
    fn recommended_freebet_plan_uses_discovered_hedge_bookmaker() {
        let opportunity = make_freebet_opportunity();
        let hunter = BonusHunter::new(shared::BonusConfig::default());

        let plan = build_recommended_freebet_plan(&hunter, &opportunity);
        assert_eq!(plan.bookmaker, "pari");
        assert_eq!(plan.hedge.bookmaker, "fonbet");
    }

    #[test]
    fn account_summary_tracks_execution_readiness() {
        let registry = ExecutionRegistry::new();
        let active_account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::SemiRealReady,
            created_at: Utc::now(),
            last_used_at: None,
        };
        let disabled_account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "fonbet".into(),
            label: "reserve".into(),
            currency: "RUB".into(),
            enabled: false,
            mode: BookmakerExecutionMode::Disabled,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(active_account.clone());
        registry.register_account(disabled_account);
        registry.upsert_session(BookmakerSession {
            account_id: active_account.id,
            bookmaker: active_account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: active_account.id,
            bookmaker: active_account.bookmaker.clone(),
            currency: active_account.currency.clone(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            captured_at: Utc::now(),
        });

        let summary = build_account_session_summary(&registry);

        assert_eq!(summary.total_bookmakers, 2);
        assert_eq!(summary.accounts_configured, 2);
        assert_eq!(summary.accounts_enabled, 1);
        assert_eq!(summary.disabled_accounts, 1);
        assert_eq!(summary.sessions_authenticated, 1);
        assert_eq!(summary.balances_cached, 1);
        assert_eq!(summary.ready_for_dry_run, 1);
        assert_eq!(summary.ready_for_execution, 1);
    }

    #[test]
    fn account_control_update_requires_explicit_confirmation() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account);

        let error = apply_account_control_update(
            &registry,
            "pari",
            AccountControlUpdateRequest {
                enabled: Some(true),
                armed: Some(true),
                confirm_dry_run_only: false,
            },
        )
        .expect_err("missing confirmation must be rejected");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("confirm_dry_run_only"));
    }

    #[test]
    fn account_control_update_only_moves_between_safe_operator_modes() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::Real,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account);

        let updated = apply_account_control_update(
            &registry,
            "pari",
            AccountControlUpdateRequest {
                enabled: Some(true),
                armed: Some(false),
                confirm_dry_run_only: true,
            },
        )
        .expect("safe downgrade should succeed");

        let account = updated.account.expect("account payload should be present");
        assert!(account.enabled);
        assert_eq!(account.mode, BookmakerExecutionMode::DryRun);
    }

    #[test]
    fn freebet_summary_aggregates_lifecycle_counts() {
        let opportunity = make_freebet_opportunity();
        let plan = FreebetConversionPlan {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            freebet_amount: 1_000.0,
            qualifying_cost: 30.0,
            conversion_rate: 0.7,
            estimated_profit: 120.0,
            hedge: shared::FreebetHedgeLeg {
                bookmaker: "fonbet".into(),
                market: "1X2".into(),
                selection: "X".into(),
                odds: 2.0,
                stake: 400.0,
            },
            steps: Vec::new(),
            created_at: Utc::now(),
        };
        let states = vec![
            FreebetLifecycleState {
                bookmaker: "pari".into(),
                lifecycle_stage: FreebetLifecycleStage::Planned,
                opportunity: Some(opportunity),
                bonus: Some(make_freebet_bonus(BonusStatus::Claimed, 0.0)),
                plan: Some(plan),
                rollover: None,
                allocation: Some(FreebetBookmakerAllocation {
                    bookmaker: "pari".into(),
                    available_balance: Some(500.0),
                    recommended_deposit: Some(250.0),
                    deposit_gap: Some(250.0),
                    urgency: Some(shared::DepositUrgency::Medium),
                    note: "top up".into(),
                }),
                updated_at: Utc::now(),
            },
            FreebetLifecycleState {
                bookmaker: "fonbet".into(),
                lifecycle_stage: FreebetLifecycleStage::RolloverCompleted,
                opportunity: None,
                bonus: None,
                plan: None,
                rollover: None,
                allocation: None,
                updated_at: Utc::now(),
            },
        ];

        let summary = build_freebet_lifecycle_summary(&states);

        assert_eq!(summary.total_bookmakers, 2);
        assert_eq!(summary.opportunities, 1);
        assert_eq!(summary.active_bonuses, 1);
        assert_eq!(summary.tracked_plans, 1);
        assert_eq!(summary.deposit_required_bookmakers, 1);
        assert_eq!(summary.planned, 1);
        assert_eq!(summary.rollover_completed, 1);
        assert_eq!(summary.total_freebet_amount, 1_000.0);
        assert_eq!(summary.total_estimated_profit, 120.0);
    }

    #[tokio::test]
    async fn stake_preflight_rejects_missing_account_state() {
        let registry = ExecutionRegistry::new();

        let response = build_stake_preflight(
            &registry,
            StakeValidationPreflightRequest {
                bookmaker: "pari".into(),
                desired_stake: 500.0,
                min_stake: None,
                max_stake: Some(1000.0),
                bankroll_available_balance: Some(1000.0),
                allow_auto_adjust: true,
            },
        )
        .await
        .expect("preflight should succeed");

        assert!(!response.executable);
        assert!(matches!(
            response.validation.decision,
            StakeValidationDecision::Reject
        ));
        assert_eq!(
            response.balance_refresh.state,
            BookmakerBalanceRefreshState::NoSession
        );
    }

    #[tokio::test]
    async fn dry_run_uses_cached_balance_and_returns_receipt() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::Real,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());
        registry.upsert_session(BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: account.currency.clone(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            captured_at: Utc::now(),
        });

        let response = build_dry_run_leg(
            &registry,
            AutoBetDryRunLegRequest {
                bookmaker: account.bookmaker.clone(),
                event_id: "event-1".into(),
                market: "1X2".into(),
                selection: "1".into(),
                odds: 2.05,
                desired_stake: 500.0,
                min_stake: Some(100.0),
                max_stake: Some(1_000.0),
                bankroll_available_balance: Some(1_000.0),
                allow_auto_adjust: true,
                reference: Some("surebet-1".into()),
            },
        )
        .await
        .expect("dry run should succeed");

        assert!(response.preflight.executable);
        assert!(response.preflight.arm_required);
        assert!(response.preflight.armed_for_execution);
        assert!(response.preflight.placement_ready);
        assert!(response.receipt.is_some());
        assert_eq!(response.execution_request.stake, 500.0);
    }

    #[tokio::test]
    async fn stake_preflight_keeps_dry_run_accounts_unarmed() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account.clone());
        registry.upsert_session(BookmakerSession {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: account.currency.clone(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            captured_at: Utc::now(),
        });

        let response = build_stake_preflight(
            &registry,
            StakeValidationPreflightRequest {
                bookmaker: account.bookmaker.clone(),
                desired_stake: 500.0,
                min_stake: Some(100.0),
                max_stake: Some(1_000.0),
                bankroll_available_balance: Some(1_000.0),
                allow_auto_adjust: true,
            },
        )
        .await
        .expect("preflight should succeed");

        assert!(response.executable);
        assert!(response.dry_run_ready);
        assert!(response.arm_required);
        assert!(!response.armed_for_execution);
        assert!(!response.placement_ready);
        assert!(!response.real_money_enabled);
    }
}
