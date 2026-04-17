use auto_betting::engine::AutoBetEngine;
use auto_betting::limiter::BetLimiterStats;
use auto_betting::validator::StakeValidator;
use auto_betting::PARI_ROLLOUT_BOOKMAKER;
use auto_betting::{ExecutionRegistry, ExecutionStatePhase, ExecutionStateReplay};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use bankroll_manager::manager::BankrollManager;
use bonus_hunter::hunter::BonusHunter;
use chrono::Utc;
use engine::freebet::FreebetHunter;
use engine::generosity::GenerosityIndexCalc;
use parsers::parser_factory::ParserFactory;
use persistence::execution_ledger::ExecutionLedgerStore;
use persistence::execution_state::ExecutionStateStore;
use persistence::freebet_lifecycle::FreebetLifecycleStore;
use persistence::history::SurebetHistory;
use scanner::freebet_lifecycle::collect_freebet_lifecycle as collect_scanner_freebet_lifecycle;
use scanner::ScannerRunner;
use serde::{Deserialize, Serialize};
use shared::models::{
    AccountSessionSummary, AutoBetDryRunLegRequest, AutoBetDryRunLegResponse, AutoBetDryRunRequest,
    AutoBetDryRunResponse, AutoBetStatus, BankrollState, BetExecutionRequest, BetPlacement,
    BetStatus, BonusInfo, BookmakerAccount, BookmakerAuthSnapshot, BookmakerBalance,
    BookmakerBalanceRefresh, BookmakerBalanceSnapshot, BookmakerExecutionCapability,
    BookmakerExecutionMode, BookmakerMetadata, BookmakerSession, DepositAllocationGuidance,
    DiagnosticSeverity, ExecutionBookmakerReadinessRecord, ExecutionBookmakerStateSummary,
    ExecutionLedgerAudit, ExecutionLedgerRecord, ExecutionOverview, ExecutionPlacementSummary,
    ExecutionStateAudit, ExecutionStateMachineMetadata, ExecutionStatePhaseSummary,
    ExecutionStateReadinessSummary, ExecutionStateSnapshotRecord, ExecutionStateTransitionRecord,
    FreebetConversionPlan, FreebetExecutionReadiness, FreebetExecutionReadinessStage,
    FreebetLifecycleFundingGapLeader, FreebetLifecycleLabelCount, FreebetLifecycleStage,
    FreebetLifecycleState, FreebetLifecycleSummary, FreebetOpportunity, FreebetPlanRequest,
    FreebetProgressStatus, FreebetRolloverProgress, GenerosityIndex, OddsError, ParserCoverage,
    ParserDiagnosticCheck, ParserHealth, ParserResultStatus, ParserRuntimeSnapshot,
    RuntimeCircuitState, ScannerMetrics, StakeValidationDecision, StakeValidationPreflightRequest,
    StakeValidationPreflightResponse, StakeValidationRequest, Surebet, ValueBet,
};
use shared::{CorridorOpportunity, ExpressFork};
use std::collections::{HashMap, HashSet};

const STATIC_PARSER_HEALTH_NOTE: &str =
    "Static factory snapshot only; runtime fetch has not been executed yet.";
const SESSION_SNAPSHOT_STALE_AFTER_SECS: i64 = 15 * 60;
const BALANCE_SNAPSHOT_STALE_AFTER_SECS: i64 = 5 * 60;
const AUTH_SNAPSHOT_STALE_AFTER_SECS: i64 = 5 * 60;

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
    pub parser_runtime_stale_after_secs: u64,
    pub parser_factory: Arc<ParserFactory>,
    pub bookmakers: Arc<Vec<BookmakerMetadata>>,
    pub history: Arc<SurebetHistory>,
    pub execution_ledger: Arc<ExecutionLedgerStore>,
    pub execution_state_store: Arc<ExecutionStateStore>,
    pub freebet_lifecycle_store: Option<Arc<FreebetLifecycleStore>>,
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
    pub auth_snapshot: Option<BookmakerAuthSnapshot>,
    pub persistence_status: AccountPersistenceStatusResponse,
    pub readiness: AccountReadinessResponse,
    pub control_issues: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AccountPersistenceStatusResponse {
    pub session_age_secs: Option<i64>,
    pub session_expires_in_secs: Option<i64>,
    pub session_stale: bool,
    pub balance_age_secs: Option<i64>,
    pub balance_stale: bool,
    pub auth_snapshot_age_secs: Option<i64>,
    pub auth_snapshot_stale: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AccountReadinessResponse {
    pub session_ready: bool,
    pub balance_ready: bool,
    pub dry_run_ready: bool,
    pub can_arm_safely: bool,
    pub placement_ready: bool,
    pub real_money_enabled: bool,
    pub rollout_gate_active: bool,
    pub approval_required: bool,
    pub submit_blocked_by_safe_mode: bool,
    pub operator_action: Option<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AccountControlUpdateRequest {
    pub enabled: Option<bool>,
    pub armed: Option<bool>,
    pub confirm_dry_run_only: bool,
    pub confirm_rollout_gate_acknowledged: Option<bool>,
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

fn sync_bankroll_with_balance_snapshot(
    bankroll_manager: &BankrollManager,
    snapshot: Option<&BookmakerBalanceSnapshot>,
) {
    if let Some(snapshot) = snapshot {
        bankroll_manager.apply_balance_snapshot(snapshot);
    }
}

fn sync_bankroll_with_registry_snapshots(
    registry: &ExecutionRegistry,
    bankroll_manager: &BankrollManager,
) {
    for snapshot in registry.list_balance_snapshots() {
        bankroll_manager.apply_balance_snapshot(&snapshot);
    }
}

fn collect_live_freebet_lifecycle(state: &AppState) -> Vec<FreebetLifecycleState> {
    collect_scanner_freebet_lifecycle(
        state.freebet_hunter.scan_freebets(),
        state.bonus_hunter.as_ref(),
        state.bankroll_manager.as_ref(),
    )
}

fn parser_health_status(
    runtime: &ParserRuntimeSnapshot,
    fallback: &ParserHealth,
) -> shared::HealthStatus {
    parser_health_status_with_freshness(runtime, fallback, Utc::now(), 0)
}

fn parser_health_status_with_freshness(
    runtime: &ParserRuntimeSnapshot,
    fallback: &ParserHealth,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> shared::HealthStatus {
    match runtime.circuit_state {
        RuntimeCircuitState::Open => shared::HealthStatus::CircuitOpen,
        RuntimeCircuitState::Closed | RuntimeCircuitState::HalfOpen => {
            if runtime.total_runs == 0 {
                return fallback.status.clone();
            }

            if runtime.is_stale(now, stale_after_secs) {
                return shared::HealthStatus::Unhealthy;
            }

            if runtime.last_success.is_none()
                || matches!(runtime.last_result_status, ParserResultStatus::Failed)
            {
                shared::HealthStatus::Unhealthy
            } else if runtime.consecutive_failures == 0
                && matches!(runtime.last_result_status, ParserResultStatus::Healthy)
            {
                shared::HealthStatus::Healthy
            } else {
                shared::HealthStatus::Degraded
            }
        }
    }
}

fn merge_parser_health(
    fallback: &ParserHealth,
    runtime: Option<&ParserRuntimeSnapshot>,
) -> ParserHealth {
    merge_parser_health_with_freshness(fallback, runtime, Utc::now(), 0)
}

fn merge_parser_health_with_freshness(
    fallback: &ParserHealth,
    runtime: Option<&ParserRuntimeSnapshot>,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> ParserHealth {
    let Some(runtime) = runtime else {
        return fallback.clone();
    };

    let has_live_runtime = runtime.last_attempt.is_some() || runtime.total_runs > 0;
    let diagnostics = if has_live_runtime {
        runtime_health_diagnostics(runtime, now, stale_after_secs)
            .into_iter()
            .collect()
    } else {
        let mut diagnostics = fallback.diagnostics.clone();
        diagnostics.extend(runtime_health_diagnostics(runtime, now, stale_after_secs));
        diagnostics
    };

    ParserHealth {
        bookmaker: fallback.bookmaker.clone(),
        status: parser_health_status_with_freshness(runtime, fallback, now, stale_after_secs),
        last_success: runtime.last_success.or(fallback.last_success),
        last_error: runtime.last_error.clone().or_else(|| {
            if has_live_runtime {
                None
            } else {
                fallback.last_error.clone()
            }
        }),
        consecutive_failures: runtime.consecutive_failures,
        avg_response_time_ms: runtime.avg_response_time_ms,
        events_parsed: runtime.events_parsed,
        uptime_percent: runtime.uptime_percent,
        readiness: fallback.readiness.clone(),
        diagnostics,
    }
}

fn runtime_only_parser_health(runtime: &ParserRuntimeSnapshot) -> ParserHealth {
    runtime_only_parser_health_with_freshness(runtime, Utc::now(), 0)
}

fn runtime_only_parser_health_with_freshness(
    runtime: &ParserRuntimeSnapshot,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> ParserHealth {
    ParserHealth {
        bookmaker: runtime.bookmaker.clone(),
        status: if matches!(runtime.circuit_state, RuntimeCircuitState::Open) {
            shared::HealthStatus::CircuitOpen
        } else if runtime.total_runs == 0 {
            shared::HealthStatus::Degraded
        } else if runtime.is_stale(now, stale_after_secs) {
            shared::HealthStatus::Unhealthy
        } else if runtime.last_success.is_none()
            || matches!(runtime.last_result_status, ParserResultStatus::Failed)
        {
            shared::HealthStatus::Unhealthy
        } else if runtime.consecutive_failures == 0
            && matches!(runtime.last_result_status, ParserResultStatus::Healthy)
        {
            shared::HealthStatus::Healthy
        } else {
            shared::HealthStatus::Degraded
        },
        last_success: runtime.last_success,
        last_error: runtime
            .last_error
            .clone()
            .or_else(|| (runtime.total_runs == 0).then(|| STATIC_PARSER_HEALTH_NOTE.to_string())),
        consecutive_failures: runtime.consecutive_failures,
        avg_response_time_ms: runtime.avg_response_time_ms,
        events_parsed: runtime.events_parsed,
        uptime_percent: runtime.uptime_percent,
        readiness: None,
        diagnostics: runtime_health_diagnostics(runtime, now, stale_after_secs)
            .into_iter()
            .collect(),
    }
}

fn runtime_health_diagnostics(
    runtime: &ParserRuntimeSnapshot,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> Vec<ParserDiagnosticCheck> {
    let state = match runtime.circuit_state {
        RuntimeCircuitState::Closed => "closed",
        RuntimeCircuitState::HalfOpen => "half_open",
        RuntimeCircuitState::Open => "open",
    };
    let last_attempt = runtime
        .last_attempt
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| "never".to_string());

    let mut diagnostics = vec![
        ParserDiagnosticCheck {
            code: "runtime_state".into(),
            severity: match runtime.circuit_state {
                RuntimeCircuitState::Closed => DiagnosticSeverity::Pass,
                RuntimeCircuitState::HalfOpen => DiagnosticSeverity::Warn,
                RuntimeCircuitState::Open => DiagnosticSeverity::Fail,
            },
            message: format!(
                "runtime circuit={state}, total_runs={}, successful_runs={}, last_attempt={last_attempt}",
                runtime.total_runs, runtime.successful_runs,
            ),
        },
        ParserDiagnosticCheck {
            code: "runtime_throughput".into(),
            severity: if runtime.total_runs == 0 {
                DiagnosticSeverity::Info
            } else if matches!(runtime.last_result_status, ParserResultStatus::Healthy) {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Warn
            },
            message: format!(
                "runtime avg_response_time_ms={:.1}, events_parsed={}, odds_parsed={}, uptime_percent={:.1}",
                runtime.avg_response_time_ms,
                runtime.events_parsed,
                runtime.odds_parsed,
                runtime.uptime_percent,
            ),
        },
    ];
    diagnostics.push(ParserDiagnosticCheck {
        code: "runtime_staleness".into(),
        severity: if runtime.total_runs == 0 {
            DiagnosticSeverity::Info
        } else if runtime.is_stale(now, stale_after_secs) {
            DiagnosticSeverity::Fail
        } else {
            DiagnosticSeverity::Pass
        },
        message: match runtime.staleness_age_secs(now) {
            Some(age_secs) if stale_after_secs > 0 => {
                format!("runtime age_secs={age_secs}, stale_after_secs={stale_after_secs}")
            }
            Some(age_secs) => format!("runtime age_secs={age_secs}, stale_after_secs=disabled"),
            None => "runtime freshness unavailable until first fetch attempt".into(),
        },
    });
    diagnostics.push(ParserDiagnosticCheck {
        code: "runtime_validation".into(),
        severity: match runtime.last_result_status {
            ParserResultStatus::Healthy => DiagnosticSeverity::Pass,
            ParserResultStatus::Degraded => DiagnosticSeverity::Warn,
            ParserResultStatus::Failed => DiagnosticSeverity::Fail,
        },
        message: runtime.last_result_message.clone().unwrap_or_else(|| {
            format!(
                "last_result_status={:?}, validation_checks={}",
                runtime.last_result_status,
                runtime.validation_checks.len()
            )
        }),
    });
    diagnostics.extend(runtime.validation_checks.clone());
    diagnostics
}

fn build_live_parsers_health(
    fallback_health: Vec<ParserHealth>,
    runtime_snapshots: Vec<ParserRuntimeSnapshot>,
) -> Vec<ParserHealth> {
    build_live_parsers_health_with_freshness(fallback_health, runtime_snapshots, Utc::now(), 0)
}

fn build_live_parsers_health_with_freshness(
    fallback_health: Vec<ParserHealth>,
    runtime_snapshots: Vec<ParserRuntimeSnapshot>,
    now: chrono::DateTime<Utc>,
    stale_after_secs: u64,
) -> Vec<ParserHealth> {
    let mut runtime = runtime_snapshots
        .into_iter()
        .map(|entry| (entry.bookmaker.clone(), entry))
        .collect::<std::collections::HashMap<_, _>>();

    let mut items = fallback_health
        .into_iter()
        .map(|fallback| {
            let runtime = runtime.remove(&fallback.bookmaker);
            merge_parser_health_with_freshness(&fallback, runtime.as_ref(), now, stale_after_secs)
        })
        .collect::<Vec<_>>();
    items.extend(
        runtime.into_values().map(|runtime| {
            runtime_only_parser_health_with_freshness(&runtime, now, stale_after_secs)
        }),
    );
    items.sort_by(|left, right| left.bookmaker.cmp(&right.bookmaker));
    items
}

fn live_parsers_health(state: &AppState) -> Vec<ParserHealth> {
    build_live_parsers_health_with_freshness(
        state.parser_factory.parser_health_snapshots(),
        state.scanner.get_parser_runtime_snapshots(),
        Utc::now(),
        state.parser_runtime_stale_after_secs,
    )
}

fn build_live_parsers_coverage(
    fallback_coverage: Vec<ParserCoverage>,
    live_health: Vec<ParserHealth>,
) -> Vec<ParserCoverage> {
    let mut live_health = live_health
        .into_iter()
        .map(|item| (item.bookmaker.clone(), item))
        .collect::<std::collections::HashMap<_, _>>();

    let mut items = fallback_coverage
        .into_iter()
        .map(|mut coverage| {
            coverage.runtime_health = live_health.remove(&coverage.slug);
            coverage
        })
        .collect::<Vec<_>>();
    items.extend(
        live_health
            .into_values()
            .map(|runtime_health| ParserCoverage {
                slug: runtime_health.bookmaker.clone(),
                name: runtime_health.bookmaker.clone(),
                enabled: true,
                scan_supported: true,
                execution_supported: false,
                status: shared::BookmakerStatus::ScanOnly,
                parser_type: "runtime".into(),
                source: "runtime".into(),
                notes: Some(
                    "Runtime-only parser slug is missing from static parser coverage registry."
                        .into(),
                ),
                readiness: runtime_health.readiness.clone(),
                runtime_health: Some(runtime_health),
            }),
    );
    items.sort_by(|left, right| left.slug.cmp(&right.slug));
    items
}

fn merge_freebet_lifecycle_state(
    primary: FreebetLifecycleState,
    secondary: FreebetLifecycleState,
) -> FreebetLifecycleState {
    FreebetLifecycleState {
        bookmaker: primary.bookmaker,
        lifecycle_stage: primary.lifecycle_stage,
        next_milestone: if primary.next_milestone.trim().is_empty() {
            secondary.next_milestone
        } else {
            primary.next_milestone
        },
        blocked_by: if primary.blocked_by.is_empty() {
            secondary.blocked_by
        } else {
            primary.blocked_by
        },
        read_only_follow_up: if primary.read_only_follow_up.trim().is_empty() {
            secondary.read_only_follow_up
        } else {
            primary.read_only_follow_up
        },
        read_only_focus: if primary.read_only_focus.trim().is_empty() {
            secondary.read_only_focus
        } else {
            primary.read_only_focus
        },
        opportunity: primary.opportunity.or(secondary.opportunity),
        bonus: primary.bonus.or(secondary.bonus),
        plan: primary.plan.or(secondary.plan),
        rollover: primary.rollover.or(secondary.rollover),
        allocation: primary.allocation.or(secondary.allocation),
        auto_rollover: primary.auto_rollover.or(secondary.auto_rollover),
        rollover_actions: if primary.rollover_actions.is_empty() {
            secondary.rollover_actions
        } else {
            primary.rollover_actions
        },
        execution_readiness: primary
            .execution_readiness
            .or(secondary.execution_readiness),
        updated_at: primary.updated_at.max(secondary.updated_at),
    }
}

fn live_parsers_coverage(state: &AppState) -> Vec<ParserCoverage> {
    build_live_parsers_coverage(
        state.parser_factory.parser_coverage(),
        live_parsers_health(state),
    )
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

fn select_freebet_lifecycle_snapshot(
    live: Vec<FreebetLifecycleState>,
    persisted: Vec<FreebetLifecycleState>,
) -> Vec<FreebetLifecycleState> {
    let mut states = persisted
        .into_iter()
        .map(|state| (state.bookmaker.clone(), state))
        .collect::<HashMap<_, _>>();

    for live_state in live {
        match states.remove(&live_state.bookmaker) {
            Some(persisted_state) => {
                let merged = if live_state.updated_at >= persisted_state.updated_at {
                    merge_freebet_lifecycle_state(live_state, persisted_state)
                } else {
                    merge_freebet_lifecycle_state(persisted_state, live_state)
                };
                states.insert(merged.bookmaker.clone(), merged);
            }
            None => {
                states.insert(live_state.bookmaker.clone(), live_state);
            }
        }
    }

    let mut items = states.into_values().collect::<Vec<_>>();
    items.sort_by(|left, right| left.bookmaker.cmp(&right.bookmaker));
    items
}

async fn load_freebet_lifecycle(state: &AppState) -> Vec<FreebetLifecycleState> {
    let live = collect_live_freebet_lifecycle(state);
    let persisted = match &state.freebet_lifecycle_store {
        Some(store) => store.list_states().await,
        None => Vec::new(),
    };

    enrich_freebet_lifecycle_bridge(
        execution_registry(state).as_ref(),
        select_freebet_lifecycle_snapshot(live, persisted),
    )
}

fn enrich_freebet_lifecycle_bridge(
    registry: &ExecutionRegistry,
    states: Vec<FreebetLifecycleState>,
) -> Vec<FreebetLifecycleState> {
    states
        .into_iter()
        .map(|state| enrich_freebet_lifecycle_state(registry, state))
        .collect()
}

fn enrich_freebet_lifecycle_state(
    registry: &ExecutionRegistry,
    mut state: FreebetLifecycleState,
) -> FreebetLifecycleState {
    if state.rollover_actions.is_empty() {
        state.rollover_actions = scanner::freebet_lifecycle::build_staged_rollover_actions(
            &state.lifecycle_stage,
            state.auto_rollover.as_ref(),
        );
    }

    state.execution_readiness = Some(build_freebet_execution_readiness(registry, &state));
    state
}

fn build_freebet_execution_readiness(
    registry: &ExecutionRegistry,
    state: &FreebetLifecycleState,
) -> FreebetExecutionReadiness {
    let capability = registry.get_capability(&state.bookmaker);
    let account = registry.get_account(&state.bookmaker);
    let session = registry.get_session(&state.bookmaker);
    let balance = registry.get_balance_snapshot(&state.bookmaker);
    let readiness = build_account_readiness(
        &state.bookmaker,
        &capability,
        account.as_ref(),
        session.as_ref(),
        balance.as_ref(),
    );
    let auto_rollover = state.auto_rollover.as_ref();
    let funding_ready = auto_rollover
        .map(|item| item.funding_readiness.ready)
        .unwrap_or(true);
    let manual_trigger_required = matches!(
        auto_rollover.map(|item| &item.status),
        Some(shared::FreebetAutoRolloverStatus::AwaitingTrigger)
    );
    let monitoring_only = matches!(
        auto_rollover.map(|item| &item.status),
        Some(shared::FreebetAutoRolloverStatus::Monitoring)
    ) || matches!(
        state.lifecycle_stage,
        FreebetLifecycleStage::RolloverInProgress
    );
    let completed = matches!(
        auto_rollover.map(|item| &item.status),
        Some(shared::FreebetAutoRolloverStatus::Completed)
    ) || matches!(
        state.lifecycle_stage,
        FreebetLifecycleStage::RolloverCompleted
    );

    let mut blocking_reasons = readiness.blocking_reasons.clone();
    if !funding_ready {
        blocking_reasons.push("freebet rollover funding gap is still open".into());
    }
    if manual_trigger_required {
        blocking_reasons.push("manual qualifying/freebet trigger is still pending".into());
    }

    let stage = if completed {
        FreebetExecutionReadinessStage::Completed
    } else if !funding_ready {
        FreebetExecutionReadinessStage::FundingBlocked
    } else if manual_trigger_required {
        FreebetExecutionReadinessStage::AwaitingManualTrigger
    } else if monitoring_only {
        FreebetExecutionReadinessStage::MonitoringOnly
    } else if account.is_some() || capability.supports_dry_run {
        FreebetExecutionReadinessStage::ReadOnlyReady
    } else {
        FreebetExecutionReadinessStage::Untracked
    };

    FreebetExecutionReadiness {
        stage,
        account_configured: account.is_some(),
        session_required: capability.requires_session,
        session_ready: readiness.session_ready,
        balance_snapshot_available: balance.is_some(),
        dry_run_ready: readiness.dry_run_ready && funding_ready && !manual_trigger_required,
        funding_ready,
        manual_trigger_required,
        monitoring_only,
        real_money_enabled: readiness.real_money_enabled,
        submit_blocked_by_safe_mode: readiness.submit_blocked_by_safe_mode,
        blocking_reasons,
    }
}

fn get_account_state(
    registry: &ExecutionRegistry,
    bookmaker: &str,
) -> Option<AccountStateResponse> {
    let known_bookmakers = registry.list_bookmakers();
    if !known_bookmakers.iter().any(|item| item == bookmaker) {
        return None;
    }

    let capability = registry.get_capability(bookmaker);
    let account = registry.get_account(bookmaker);
    let session = registry.get_session(bookmaker);
    let balance = registry.get_balance_snapshot(bookmaker);
    let auth_snapshot = registry.get_auth_snapshot(bookmaker);
    let persistence_status = build_account_persistence_status(
        bookmaker,
        session.as_ref(),
        balance.as_ref(),
        auth_snapshot.as_ref(),
        Utc::now(),
    );
    let readiness = build_account_readiness(
        bookmaker,
        &capability,
        account.as_ref(),
        session.as_ref(),
        balance.as_ref(),
    );

    Some(AccountStateResponse {
        bookmaker: bookmaker.to_string(),
        readiness,
        control_issues: collect_account_control_issues(
            account.as_ref(),
            &capability,
            session.as_ref(),
            balance.as_ref(),
        ),
        capability,
        account,
        session,
        balance,
        auth_snapshot,
        persistence_status,
    })
}

fn age_secs_since(timestamp: chrono::DateTime<Utc>, now: chrono::DateTime<Utc>) -> i64 {
    now.signed_duration_since(timestamp).num_seconds().max(0)
}

fn expires_in_secs(
    expires_at: Option<chrono::DateTime<Utc>>,
    now: chrono::DateTime<Utc>,
) -> Option<i64> {
    expires_at.map(|timestamp| timestamp.signed_duration_since(now).num_seconds())
}

fn build_account_persistence_status(
    bookmaker: &str,
    session: Option<&BookmakerSession>,
    balance: Option<&BookmakerBalanceSnapshot>,
    auth_snapshot: Option<&BookmakerAuthSnapshot>,
    now: chrono::DateTime<Utc>,
) -> AccountPersistenceStatusResponse {
    let session_age_secs = session.map(|item| age_secs_since(item.last_synced_at, now));
    let session_expires_in_secs = expires_in_secs(session.and_then(|item| item.expires_at), now);
    let balance_age_secs = balance.map(|item| age_secs_since(item.captured_at, now));
    let auth_snapshot_age_secs = auth_snapshot.map(|item| age_secs_since(item.captured_at, now));

    let session_stale = session_age_secs
        .map(|age| age > SESSION_SNAPSHOT_STALE_AFTER_SECS)
        .unwrap_or(false)
        || session_expired_locally(session, now);
    let balance_stale = balance_age_secs
        .map(|age| age > BALANCE_SNAPSHOT_STALE_AFTER_SECS)
        .unwrap_or(false);
    let auth_snapshot_stale_by_age = auth_snapshot_age_secs
        .map(|age| age > AUTH_SNAPSHOT_STALE_AFTER_SECS)
        .unwrap_or(false);
    let auth_snapshot_behind_session = auth_snapshot
        .zip(session)
        .map(|(auth_snapshot, session)| auth_snapshot.captured_at < session.last_synced_at)
        .unwrap_or(false);
    let auth_snapshot_behind_balance = auth_snapshot
        .zip(balance)
        .map(|(auth_snapshot, balance)| auth_snapshot.captured_at < balance.captured_at)
        .unwrap_or(false);
    let auth_snapshot_stale =
        auth_snapshot_stale_by_age || auth_snapshot_behind_session || auth_snapshot_behind_balance;

    let mut warnings = Vec::new();

    if session_stale {
        warnings.push(format!(
            "{bookmaker} persisted session snapshot is stale; refresh auth/session sync before relying on cached readiness"
        ));
    }

    if balance_stale {
        warnings.push(format!(
            "{bookmaker} persisted balance snapshot is stale; refresh balance-dependent operator checks"
        ));
    }

    if auth_snapshot_behind_session || auth_snapshot_behind_balance {
        warnings.push(format!(
            "{bookmaker} auth readiness snapshot lags behind newer persisted session/balance state"
        ));
    } else if auth_snapshot_stale_by_age {
        warnings.push(format!(
            "{bookmaker} auth readiness snapshot is stale; recompute operator readiness before arming"
        ));
    }

    AccountPersistenceStatusResponse {
        session_age_secs,
        session_expires_in_secs,
        session_stale,
        balance_age_secs,
        balance_stale,
        auth_snapshot_age_secs,
        auth_snapshot_stale,
        warnings,
    }
}

fn session_expired_locally(session: Option<&BookmakerSession>, now: chrono::DateTime<Utc>) -> bool {
    session
        .and_then(|item| {
            matches!(item.state, shared::BookmakerSessionState::Active).then_some(item.expires_at)
        })
        .flatten()
        .map(|expires_at| expires_at <= now)
        .unwrap_or(false)
}

fn session_is_authenticated_now(
    session: Option<&BookmakerSession>,
    now: chrono::DateTime<Utc>,
) -> bool {
    session
        .map(|item| matches!(item.state, shared::BookmakerSessionState::Active))
        .unwrap_or(false)
        && !session_expired_locally(session, now)
}

fn build_account_readiness(
    bookmaker: &str,
    capability: &BookmakerExecutionCapability,
    account: Option<&BookmakerAccount>,
    session: Option<&BookmakerSession>,
    balance: Option<&BookmakerBalanceSnapshot>,
) -> AccountReadinessResponse {
    let now = Utc::now();
    let session_ready = !capability.requires_session || session_is_authenticated_now(session, now);
    let balance_ready = !capability.supports_balance_snapshot || balance.is_some();
    let dry_run_ready = account
        .map(|item| item.enabled && item.mode.allows_dry_run())
        .unwrap_or(false)
        && capability.supports_dry_run
        && session_ready
        && balance_ready;
    let can_arm_safely = account.map(|item| item.enabled).unwrap_or(false)
        && capability.supports_dry_run
        && capability.supports_bet_placement
        && session_ready
        && balance_ready;
    let placement_ready = account
        .map(|item| item.enabled && item.mode.allows_submission_path())
        .unwrap_or(false)
        && capability.supports_bet_placement
        && session_ready
        && balance_ready;
    let real_money_enabled = placement_ready && capability.supports_real_money;
    let rollout_gate_active = bookmaker == PARI_ROLLOUT_BOOKMAKER
        && capability.supports_bet_placement
        && !capability.supports_real_money;
    let approval_required = rollout_gate_active && placement_ready;
    let submit_blocked_by_safe_mode = approval_required && !real_money_enabled;

    let mut blocking_reasons = Vec::new();

    if account.is_none() {
        blocking_reasons.push("bookmaker account is not configured".into());
    }

    if let Some(account) = account {
        if !account.enabled {
            blocking_reasons.push("bookmaker account is disabled".into());
        }

        if matches!(account.mode, BookmakerExecutionMode::Disabled) {
            blocking_reasons.push("bookmaker account mode is disabled".into());
        }
    }

    if capability.requires_session && !session_ready {
        blocking_reasons.push("active bookmaker session is required".into());
    }

    if capability.requires_session && session_expired_locally(session, now) {
        blocking_reasons.push("bookmaker session expiry timestamp has passed".into());
    }

    if capability.supports_balance_snapshot && !balance_ready {
        blocking_reasons.push("cached bookmaker balance snapshot is required".into());
    }

    if !capability.supports_bet_placement {
        blocking_reasons.push("bookmaker adapter does not support placement path".into());
    }

    if rollout_gate_active {
        blocking_reasons.push(
            "pari rollout gate remains active: operator approval may be recorded, but coupon submit stays disabled"
                .into(),
        );
    }

    let operator_action = if capability.requires_session && session_expired_locally(session, now) {
        Some(format!(
            "refresh the {bookmaker} session before enabling dry-run or balance-dependent checks"
        ))
    } else if submit_blocked_by_safe_mode {
        Some(
            "record operator approval for pari rollout monitoring; coupon submit remains disabled in safe mode"
                .into(),
        )
    } else if rollout_gate_active && !session_ready {
        Some("refresh the pari session before arming the rollout path".into())
    } else if rollout_gate_active && !balance_ready {
        Some("capture a fresh pari balance snapshot before arming the rollout path".into())
    } else {
        None
    };

    AccountReadinessResponse {
        session_ready,
        balance_ready,
        dry_run_ready,
        can_arm_safely,
        placement_ready,
        real_money_enabled,
        rollout_gate_active,
        approval_required,
        submit_blocked_by_safe_mode,
        operator_action,
        blocking_reasons,
    }
}

fn collect_account_control_issues(
    account: Option<&BookmakerAccount>,
    capability: &BookmakerExecutionCapability,
    session: Option<&BookmakerSession>,
    balance: Option<&BookmakerBalanceSnapshot>,
) -> Vec<String> {
    let Some(account) = account else {
        return Vec::new();
    };

    let mut issues = Vec::new();
    let now = Utc::now();

    if account.enabled && matches!(account.mode, BookmakerExecutionMode::Disabled) {
        issues.push("account is enabled but execution mode is disabled".into());
    }

    if !account.enabled && account.mode.allows_dry_run() {
        issues.push("account is disabled but mode still allows execution paths".into());
    }

    if account.mode.is_armed() && !capability.supports_bet_placement {
        issues.push("account is armed but bookmaker adapter cannot place bets".into());
    }

    if account.enabled && capability.requires_session && !session_is_authenticated_now(session, now)
    {
        issues.push("enabled account requires an active session".into());
    }

    if account.enabled && capability.requires_session && session_expired_locally(session, now) {
        issues.push("enabled account has a locally expired session timestamp".into());
    }

    if account.enabled && capability.supports_balance_snapshot && balance.is_none() {
        issues.push("enabled account is missing a cached balance snapshot".into());
    }

    issues
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
    let capability = registry.get_capability(bookmaker);

    if bookmaker == PARI_ROLLOUT_BOOKMAKER
        && enabled
        && armed
        && capability.supports_bet_placement
        && !capability.supports_real_money
        && request.confirm_rollout_gate_acknowledged != Some(true)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "arming pari rollout path requires confirm_rollout_gate_acknowledged=true".into(),
        ));
    }

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
        accounts_with_control_issues: 0,
        sessions_configured: 0,
        sessions_authenticated: 0,
        sessions_stale: 0,
        balances_cached: 0,
        balances_stale: 0,
        auth_snapshots_stale: 0,
        ready_for_execution: 0,
        ready_for_dry_run: 0,
    };

    let now = Utc::now();

    for bookmaker in registry.list_bookmakers() {
        summary.total_bookmakers += 1;

        let capability = registry.get_capability(&bookmaker);
        let account = registry.get_account(&bookmaker);
        let session = registry.get_session(&bookmaker);
        let balance = registry.get_balance_snapshot(&bookmaker);
        let auth_snapshot = registry.get_auth_snapshot(&bookmaker);
        let persistence_status = build_account_persistence_status(
            &bookmaker,
            session.as_ref(),
            balance.as_ref(),
            auth_snapshot.as_ref(),
            now,
        );
        let control_issues = collect_account_control_issues(
            account.as_ref(),
            &capability,
            session.as_ref(),
            balance.as_ref(),
        );

        if let Some(account) = account.as_ref() {
            summary.accounts_configured += 1;
            if account.enabled {
                summary.accounts_enabled += 1;
            } else {
                summary.disabled_accounts += 1;
            }
        }

        if !control_issues.is_empty() {
            summary.accounts_with_control_issues += 1;
        }

        if session.is_some() {
            summary.sessions_configured += 1;
        }

        if session_is_authenticated_now(session.as_ref(), now) {
            summary.sessions_authenticated += 1;
        }

        if persistence_status.session_stale {
            summary.sessions_stale += 1;
        }

        if balance.is_some() {
            summary.balances_cached += 1;
        }

        if persistence_status.balance_stale {
            summary.balances_stale += 1;
        }

        if persistence_status.auth_snapshot_stale {
            summary.auth_snapshots_stale += 1;
        }

        let executable = account
            .as_ref()
            .map(|item| {
                item.enabled && !matches!(item.mode, shared::BookmakerExecutionMode::Disabled)
            })
            .unwrap_or(false);
        let session_ready =
            !capability.requires_session || session_is_authenticated_now(session.as_ref(), now);
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

fn merge_execution_placements(
    runtime_placements: Vec<BetPlacement>,
    ledger_placements: Vec<BetPlacement>,
) -> Vec<BetPlacement> {
    let capacity = runtime_placements.len() + ledger_placements.len();
    let mut merged = Vec::with_capacity(capacity);
    let mut seen = HashSet::new();

    for placement in runtime_placements.into_iter().chain(ledger_placements) {
        if seen.insert(placement.id) {
            merged.push(placement);
        }
    }

    merged
}

async fn load_recent_execution_placements(state: &AppState, limit: usize) -> Vec<BetPlacement> {
    let runtime_placements = state.auto_bet_engine.get_history(limit);
    let ledger_placements = state
        .execution_ledger
        .get_recent_placements(limit)
        .await
        .unwrap_or_default();

    let mut placements = merge_execution_placements(runtime_placements, ledger_placements);
    placements.truncate(limit);
    placements
}

async fn build_execution_overview(state: &AppState) -> ExecutionOverview {
    let registry = execution_registry(state);
    let placements = load_recent_execution_placements(state, 100).await;
    let ledger_placements = state
        .execution_ledger
        .summarize_latest_placements()
        .await
        .unwrap_or_else(|_| build_execution_placement_summary(&placements));
    let mut autobet_status = state.auto_bet_engine.get_status();

    if autobet_status.last_bet.is_none() {
        autobet_status.last_bet = placements.first().map(|placement| placement.placed_at);
    }

    ExecutionOverview {
        autobet_status,
        accounts: build_account_session_summary(&registry),
        recent_placements: build_execution_placement_summary(&placements),
        ledger_placements,
        state_machine: load_execution_state_metadata(state, 5).await,
        generated_at: Utc::now(),
    }
}

fn build_execution_ledger_audit(
    entries: Vec<auto_betting::ExecutionLedgerEntry>,
    state_machine: ExecutionStateMachineMetadata,
) -> ExecutionLedgerAudit {
    let latest_recorded_at = entries.first().map(|entry| entry.recorded_at);
    let unique_placements = entries
        .iter()
        .map(|entry| entry.placement.id)
        .collect::<HashSet<_>>()
        .len();
    let recent_records: Vec<ExecutionLedgerRecord> = entries
        .into_iter()
        .map(|entry| ExecutionLedgerRecord {
            placement: entry.placement,
            action: format!("{:?}", entry.action),
            recorded_at: entry.recorded_at,
        })
        .collect();

    ExecutionLedgerAudit {
        total_entries: recent_records.len(),
        unique_placements,
        latest_recorded_at,
        state_machine,
        recent_records,
        generated_at: Utc::now(),
    }
}

fn build_execution_state_metadata(
    replay: ExecutionStateReplay,
    recent_limit: usize,
) -> ExecutionStateMachineMetadata {
    let latest_snapshot_at = replay.snapshots.iter().map(|item| item.updated_at).max();
    let latest_transition_at = replay.transitions.iter().map(|item| item.occurred_at).max();
    let mut phases = ExecutionStatePhaseSummary::default();

    for snapshot in &replay.snapshots {
        match snapshot.phase {
            ExecutionStatePhase::PendingPlacement => phases.pending_placement += 1,
            ExecutionStatePhase::ConfirmedPlacement => phases.confirmed_placement += 1,
            ExecutionStatePhase::Settled => phases.settled += 1,
            ExecutionStatePhase::Cancelled => phases.cancelled += 1,
            ExecutionStatePhase::Failed => phases.failed += 1,
        }
    }

    let mut recent_snapshots = replay.snapshots;
    recent_snapshots.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    recent_snapshots.truncate(recent_limit);

    ExecutionStateMachineMetadata {
        total_snapshots: phases.pending_placement
            + phases.confirmed_placement
            + phases.settled
            + phases.cancelled
            + phases.failed,
        total_transitions: replay.transitions.len(),
        latest_snapshot_at,
        latest_transition_at,
        phases,
        recent_snapshots: recent_snapshots
            .into_iter()
            .map(|snapshot| ExecutionStateSnapshotRecord {
                placement_id: snapshot.placement_id,
                bookmaker: snapshot.bookmaker,
                phase: format!("{:?}", snapshot.phase),
                placement_status: snapshot.placement_status,
                sequence: snapshot.sequence,
                updated_at: snapshot.updated_at,
                last_action: format!("{:?}", snapshot.last_action),
                last_error: snapshot.last_error,
            })
            .collect(),
    }
}

fn increment_execution_phase(summary: &mut ExecutionStatePhaseSummary, phase: ExecutionStatePhase) {
    match phase {
        ExecutionStatePhase::PendingPlacement => summary.pending_placement += 1,
        ExecutionStatePhase::ConfirmedPlacement => summary.confirmed_placement += 1,
        ExecutionStatePhase::Settled => summary.settled += 1,
        ExecutionStatePhase::Cancelled => summary.cancelled += 1,
        ExecutionStatePhase::Failed => summary.failed += 1,
    }
}

fn build_execution_state_readiness(
    registry: &ExecutionRegistry,
) -> (
    ExecutionStateReadinessSummary,
    Vec<ExecutionBookmakerReadinessRecord>,
) {
    let mut summary = ExecutionStateReadinessSummary::default();
    let mut bookmaker_readiness = Vec::new();

    for bookmaker in registry.list_bookmakers() {
        let Some(state) = get_account_state(registry, &bookmaker) else {
            continue;
        };

        summary.total_bookmakers += 1;
        summary.accounts_configured += usize::from(state.account.is_some());
        summary.accounts_enabled += usize::from(
            state
                .account
                .as_ref()
                .map(|account| account.enabled)
                .unwrap_or(false),
        );
        let session_authenticated = state
            .session
            .as_ref()
            .map(|session| session_is_authenticated_now(Some(session), Utc::now()))
            .unwrap_or(false);

        summary.auth_ready += usize::from(state.readiness.session_ready);
        summary.sessions_authenticated += usize::from(session_authenticated);
        summary.sessions_stale += usize::from(state.persistence_status.session_stale);
        summary.balances_cached += usize::from(state.balance.is_some());
        summary.balances_stale += usize::from(state.persistence_status.balance_stale);
        summary.auth_snapshots_stale += usize::from(state.persistence_status.auth_snapshot_stale);
        summary.dry_run_ready += usize::from(state.readiness.dry_run_ready);
        summary.placement_ready += usize::from(state.readiness.placement_ready);
        summary.approval_required += usize::from(state.readiness.approval_required);
        summary.submit_blocked_by_safe_mode +=
            usize::from(state.readiness.submit_blocked_by_safe_mode);
        summary.operator_attention_required += usize::from(
            state.readiness.approval_required || state.readiness.operator_action.is_some(),
        );

        bookmaker_readiness.push(ExecutionBookmakerReadinessRecord {
            bookmaker: state.bookmaker,
            account_configured: state.account.is_some(),
            account_enabled: state
                .account
                .as_ref()
                .map(|account| account.enabled)
                .unwrap_or(false),
            execution_mode: state.account.as_ref().map(|account| account.mode.clone()),
            requires_session: state.capability.requires_session,
            auth_ready: state.readiness.session_ready,
            session_authenticated,
            session_stale: state.persistence_status.session_stale,
            balance_cached: state.balance.is_some(),
            balance_stale: state.persistence_status.balance_stale,
            auth_snapshot_stale: state.persistence_status.auth_snapshot_stale,
            dry_run_ready: state.readiness.dry_run_ready,
            placement_ready: state.readiness.placement_ready,
            approval_required: state.readiness.approval_required,
            submit_blocked_by_safe_mode: state.readiness.submit_blocked_by_safe_mode,
            persistence_warnings: state.persistence_status.warnings.clone(),
            operator_action: state.readiness.operator_action.clone(),
            blocking_reasons: state.readiness.blocking_reasons.clone(),
        });
    }

    bookmaker_readiness.sort_by(|left, right| {
        right
            .approval_required
            .cmp(&left.approval_required)
            .then_with(|| right.auth_ready.cmp(&left.auth_ready))
            .then_with(|| left.bookmaker.cmp(&right.bookmaker))
    });

    (summary, bookmaker_readiness)
}

fn build_execution_state_audit(
    registry: &ExecutionRegistry,
    replay: ExecutionStateReplay,
    recent_limit: usize,
) -> ExecutionStateAudit {
    let latest_snapshot_at = replay.snapshots.iter().map(|item| item.updated_at).max();
    let latest_transition_at = replay.transitions.iter().map(|item| item.occurred_at).max();
    let total_transitions = replay.transitions.len();
    let mut bookmaker_summaries = HashMap::<String, ExecutionBookmakerStateSummary>::new();
    let mut bookmaker_latest_error_at = HashMap::<String, chrono::DateTime<Utc>>::new();
    let (readiness, bookmaker_readiness) = build_execution_state_readiness(registry);

    for snapshot in &replay.snapshots {
        let summary = bookmaker_summaries
            .entry(snapshot.bookmaker.clone())
            .or_insert_with(|| ExecutionBookmakerStateSummary {
                bookmaker: snapshot.bookmaker.clone(),
                ..ExecutionBookmakerStateSummary::default()
            });
        summary.total_snapshots += 1;
        increment_execution_phase(&mut summary.phases, snapshot.phase);
        summary.latest_snapshot_at = Some(
            summary
                .latest_snapshot_at
                .map(|current| current.max(snapshot.updated_at))
                .unwrap_or(snapshot.updated_at),
        );

        if let Some(last_error) = &snapshot.last_error {
            let replace = bookmaker_latest_error_at
                .get(&snapshot.bookmaker)
                .map(|current| snapshot.updated_at >= *current)
                .unwrap_or(true);
            if replace {
                summary.latest_error = Some(last_error.clone());
                bookmaker_latest_error_at.insert(snapshot.bookmaker.clone(), snapshot.updated_at);
            }
        }
    }

    for transition in &replay.transitions {
        let summary = bookmaker_summaries
            .entry(transition.bookmaker.clone())
            .or_insert_with(|| ExecutionBookmakerStateSummary {
                bookmaker: transition.bookmaker.clone(),
                ..ExecutionBookmakerStateSummary::default()
            });
        summary.latest_transition_at = Some(
            summary
                .latest_transition_at
                .map(|current| current.max(transition.occurred_at))
                .unwrap_or(transition.occurred_at),
        );

        if let Some(error) = &transition.error {
            let replace = bookmaker_latest_error_at
                .get(&transition.bookmaker)
                .map(|current| transition.occurred_at >= *current)
                .unwrap_or(true);
            if replace {
                summary.latest_error = Some(error.clone());
                bookmaker_latest_error_at
                    .insert(transition.bookmaker.clone(), transition.occurred_at);
            }
        }
    }

    let mut bookmaker_summaries = bookmaker_summaries.into_values().collect::<Vec<_>>();
    bookmaker_summaries.sort_by(|left, right| {
        right
            .latest_transition_at
            .or(right.latest_snapshot_at)
            .cmp(&left.latest_transition_at.or(left.latest_snapshot_at))
            .then_with(|| left.bookmaker.cmp(&right.bookmaker))
    });

    let mut recent_transitions = replay.transitions;
    recent_transitions.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    recent_transitions.truncate(recent_limit);

    ExecutionStateAudit {
        total_snapshots: replay.snapshots.len(),
        total_transitions,
        latest_snapshot_at,
        latest_transition_at,
        readiness,
        bookmaker_readiness,
        bookmaker_summaries,
        recent_transitions: recent_transitions
            .into_iter()
            .map(|transition| ExecutionStateTransitionRecord {
                placement_id: transition.placement_id,
                bookmaker: transition.bookmaker,
                from_phase: transition.from_phase.map(|phase| format!("{:?}", phase)),
                to_phase: format!("{:?}", transition.to_phase),
                placement_status: transition.placement_status,
                sequence: transition.sequence,
                action: format!("{:?}", transition.action),
                occurred_at: transition.occurred_at,
                error: transition.error,
            })
            .collect(),
        generated_at: Utc::now(),
    }
}

async fn load_execution_state_metadata(
    state: &AppState,
    recent_limit: usize,
) -> ExecutionStateMachineMetadata {
    let snapshots = state.execution_state_store.load_state_snapshots().await;
    let transitions = state.execution_state_store.load_transitions().await;

    if !snapshots.is_empty() || !transitions.is_empty() {
        return build_execution_state_metadata(
            ExecutionStateReplay {
                snapshots,
                transitions,
            },
            recent_limit,
        );
    }

    state
        .execution_ledger
        .replay_state_machine()
        .await
        .map(|replay| build_execution_state_metadata(replay, recent_limit))
        .unwrap_or_default()
}

async fn load_execution_state_replay(state: &AppState) -> ExecutionStateReplay {
    let snapshots = state.execution_state_store.load_state_snapshots().await;
    let transitions = state.execution_state_store.load_transitions().await;

    if !snapshots.is_empty() || !transitions.is_empty() {
        return ExecutionStateReplay {
            snapshots,
            transitions,
        };
    }

    state
        .execution_ledger
        .replay_state_machine()
        .await
        .unwrap_or_default()
}

fn build_freebet_lifecycle_summary(states: &[FreebetLifecycleState]) -> FreebetLifecycleSummary {
    fn sorted_label_counts(counts: HashMap<String, usize>) -> Vec<FreebetLifecycleLabelCount> {
        let mut labels: Vec<_> = counts
            .into_iter()
            .map(|(label, count)| FreebetLifecycleLabelCount { label, count })
            .collect();
        labels.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.label.cmp(&right.label))
        });
        labels
    }

    let mut summary = FreebetLifecycleSummary {
        total_bookmakers: states.len(),
        opportunities: 0,
        active_bonuses: 0,
        tracked_plans: 0,
        deposit_required_bookmakers: 0,
        blocked_states: 0,
        total_funding_gap: 0.0,
        largest_funding_gap: None,
        discovered: 0,
        available: 0,
        qualified: 0,
        planned: 0,
        rollover_in_progress: 0,
        rollover_completed: 0,
        next_milestones: Vec::new(),
        blockers: Vec::new(),
        read_only_focuses: Vec::new(),
        total_freebet_amount: 0.0,
        total_estimated_profit: 0.0,
        generated_at: Utc::now(),
    };
    let mut next_milestones = HashMap::new();
    let mut blockers = HashMap::new();
    let mut read_only_focuses = HashMap::new();

    for state in states {
        if !state.next_milestone.is_empty() {
            *next_milestones
                .entry(state.next_milestone.clone())
                .or_insert(0usize) += 1;
        }
        for blocker in &state.blocked_by {
            *blockers.entry(blocker.clone()).or_insert(0usize) += 1;
        }
        if !state.read_only_focus.is_empty() {
            *read_only_focuses
                .entry(state.read_only_focus.clone())
                .or_insert(0usize) += 1;
        }

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

        if !state.blocked_by.is_empty() {
            summary.blocked_states += 1;
        }

        if let Some(auto_rollover) = state.auto_rollover.as_ref() {
            summary.total_funding_gap += auto_rollover.funding_readiness.total_gap;
            if let Some(amount) = auto_rollover.funding_readiness.largest_gap_amount {
                let bookmaker = auto_rollover
                    .funding_readiness
                    .largest_gap_bookmaker
                    .clone()
                    .unwrap_or_else(|| state.bookmaker.clone());
                let replace_current = summary
                    .largest_funding_gap
                    .as_ref()
                    .map(|current| {
                        amount > current.amount
                            || (amount == current.amount && bookmaker < current.bookmaker)
                    })
                    .unwrap_or(true);
                if replace_current {
                    summary.largest_funding_gap =
                        Some(FreebetLifecycleFundingGapLeader { bookmaker, amount });
                }
            }
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

    summary.next_milestones = sorted_label_counts(next_milestones);
    summary.blockers = sorted_label_counts(blockers);
    summary.read_only_focuses = sorted_label_counts(read_only_focuses);

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
    let rollout_gate_active = bookmaker == PARI_ROLLOUT_BOOKMAKER
        && capability.supports_bet_placement
        && !capability.supports_real_money;
    let approval_required = rollout_gate_active && placement_ready;
    let submit_blocked_by_safe_mode = approval_required && !real_money_enabled;
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
        rollout_gate_active,
        approval_required,
        submit_blocked_by_safe_mode,
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
    Json(ApiResponse::ok(load_freebet_lifecycle(&state).await))
}

pub async fn get_freebet_summary(
    State(state): State<AppState>,
) -> Json<ApiResponse<FreebetLifecycleSummary>> {
    let lifecycle = load_freebet_lifecycle(&state).await;
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

pub async fn get_odds_errors(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Json<ApiResponse<Vec<OddsError>>> {
    let limit = params.limit.unwrap_or(50) as usize;
    Json(ApiResponse::ok(state.scanner.get_odds_errors(limit)))
}

pub async fn get_generosity(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<GenerosityIndex>>> {
    let indices = state.generosity_index.get_all_indices();
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
            "total_profit": stats.total_profit,
        })))),
        Err(e) => {
            tracing::error!(error = e.to_string(), "Failed to get stats");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_history(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Result<Json<ApiResponse<Vec<Surebet>>>, StatusCode> {
    let limit = params.limit.unwrap_or(50);
    match state.history.get_recent(limit).await {
        Ok(history) => Ok(Json(ApiResponse::ok(history))),
        Err(e) => {
            tracing::error!(
                error = e.to_string(),
                limit,
                "Failed to get surebet history"
            );
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
    Json(ApiResponse::ok(build_execution_overview(&state).await))
}

pub async fn get_execution_ledger(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Json<ApiResponse<ExecutionLedgerAudit>> {
    let limit = params.limit.unwrap_or(50) as usize;
    let entries = state
        .execution_ledger
        .get_recent(limit)
        .await
        .unwrap_or_default();
    let state_machine = load_execution_state_metadata(&state, 5).await;
    Json(ApiResponse::ok(build_execution_ledger_audit(
        entries,
        state_machine,
    )))
}

pub async fn get_execution_state(
    State(state): State<AppState>,
    Query(params): Query<SurebetsQuery>,
) -> Json<ApiResponse<ExecutionStateAudit>> {
    let limit = params.limit.unwrap_or(50).max(1) as usize;
    let replay = load_execution_state_replay(&state).await;
    let registry = execution_registry(&state);
    Json(ApiResponse::ok(build_execution_state_audit(
        registry.as_ref(),
        replay,
        limit,
    )))
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
    Json(ApiResponse::ok(
        load_recent_execution_placements(&state, limit).await,
    ))
}

pub async fn get_bankroll(State(state): State<AppState>) -> Json<ApiResponse<BankrollState>> {
    let registry = execution_registry(&state);
    sync_bankroll_with_registry_snapshots(&registry, state.bankroll_manager.as_ref());
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
    Json(ApiResponse::ok(live_parsers_coverage(&state)))
}

pub async fn get_parsers_health(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ParserHealth>>> {
    Json(ApiResponse::ok(live_parsers_health(&state)))
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
        Ok(snapshot) => {
            sync_bankroll_with_balance_snapshot(
                state.bankroll_manager.as_ref(),
                snapshot.snapshot.as_ref(),
            );
            (StatusCode::OK, Json(ApiResponse::ok(snapshot))).into_response()
        }
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
    let parser_coverage = live_parsers_coverage(&state);

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
            current_surface: vec!["GET /api/v1/surebets", "GET /api/v1/freebets", "GET /api/v1/corridors", "GET /api/v1/express-forks", "GET /api/v1/history", "GET /api/v1/history/stats", "GET /api/v1/bookmakers", "GET /ws"],
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
    use chrono::{Duration, Utc};
    use persistence::freebet_lifecycle::FreebetLifecycleStore;
    use scanner::ParserRuntimeSnapshot;
    use shared::{
        BonusDifficulty, BonusStatus, BonusType, BookmakerAccount, BookmakerBalanceRefreshState,
        BookmakerExecutionMode, BookmakerSession, BookmakerSessionState, DiagnosticSeverity, Event,
        FreebetAutoRolloverDraft, FreebetAutoRolloverStatus, FreebetFundingReadiness, HealthStatus,
        ParserDiagnosticCheck, ParserReadiness, ParserReadinessStage, ParserResultStatus, Sport,
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

    fn make_snapshot_health(bookmaker: &str) -> ParserHealth {
        ParserHealth {
            bookmaker: bookmaker.into(),
            status: HealthStatus::Degraded,
            last_success: None,
            last_error: Some(STATIC_PARSER_HEALTH_NOTE.into()),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: None,
            diagnostics: Vec::new(),
        }
    }

    fn make_execution_snapshot(
        placement_id: Uuid,
        bookmaker: &str,
        phase: ExecutionStatePhase,
        status: BetStatus,
        sequence: u64,
        updated_at: chrono::DateTime<Utc>,
        last_error: Option<&str>,
    ) -> auto_betting::ExecutionStateSnapshot {
        auto_betting::ExecutionStateSnapshot {
            placement_id,
            bookmaker: bookmaker.into(),
            phase,
            placement_status: status,
            sequence,
            updated_at,
            last_action: auto_betting::ExecutionLedgerAction::Updated,
            last_error: last_error.map(str::to_string),
        }
    }

    fn make_execution_transition(
        placement_id: Uuid,
        bookmaker: &str,
        from_phase: Option<ExecutionStatePhase>,
        to_phase: ExecutionStatePhase,
        status: BetStatus,
        sequence: u64,
        occurred_at: chrono::DateTime<Utc>,
        error: Option<&str>,
    ) -> auto_betting::ExecutionStateTransition {
        auto_betting::ExecutionStateTransition {
            placement_id,
            bookmaker: bookmaker.into(),
            from_phase,
            to_phase,
            placement_status: status,
            sequence,
            action: auto_betting::ExecutionLedgerAction::Updated,
            occurred_at,
            error: error.map(str::to_string),
        }
    }

    #[test]
    fn merge_parser_health_prefers_runtime_success_metrics() {
        let fallback = make_snapshot_health("pari");
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
            last_attempt: Some(Utc::now()),
            last_success: Some(Utc::now()),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 42.0,
            events_parsed: 123,
            odds_parsed: 456,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        };

        let merged = merge_parser_health(&fallback, Some(&runtime));

        assert!(matches!(merged.status, HealthStatus::Healthy));
        assert_eq!(merged.avg_response_time_ms, 42.0);
        assert_eq!(merged.events_parsed, 123);
        assert_eq!(merged.uptime_percent, 100.0);
        assert!(merged.last_success.is_some());
        assert_eq!(merged.last_error, None);
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_state"
                && matches!(check.severity, DiagnosticSeverity::Pass)
                && check.message.contains("total_runs=1")
                && check.message.contains("successful_runs=1")
        }));
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_throughput"
                && matches!(check.severity, DiagnosticSeverity::Pass)
                && check.message.contains("events_parsed=123")
        }));
        assert!(merged
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_validation"));
    }

    #[test]
    fn execution_state_audit_exposes_recent_transitions_and_bookmaker_rollup() {
        let now = Utc::now();
        let pari_id = Uuid::new_v4();
        let fonbet_id = Uuid::new_v4();
        let registry = ExecutionRegistry::new();
        let replay = ExecutionStateReplay {
            snapshots: vec![
                make_execution_snapshot(
                    pari_id,
                    "pari",
                    ExecutionStatePhase::Failed,
                    BetStatus::Error,
                    2,
                    now - Duration::seconds(5),
                    Some("coupon rejected"),
                ),
                make_execution_snapshot(
                    fonbet_id,
                    "fonbet",
                    ExecutionStatePhase::Settled,
                    BetStatus::Settled,
                    2,
                    now - Duration::seconds(10),
                    None,
                ),
            ],
            transitions: vec![
                make_execution_transition(
                    fonbet_id,
                    "fonbet",
                    Some(ExecutionStatePhase::ConfirmedPlacement),
                    ExecutionStatePhase::Settled,
                    BetStatus::Settled,
                    2,
                    now - Duration::seconds(10),
                    None,
                ),
                make_execution_transition(
                    pari_id,
                    "pari",
                    Some(ExecutionStatePhase::ConfirmedPlacement),
                    ExecutionStatePhase::Failed,
                    BetStatus::Error,
                    2,
                    now - Duration::seconds(1),
                    Some("coupon rejected"),
                ),
            ],
        };

        audit_ready_bookmaker(&registry, "pari", BookmakerExecutionMode::SemiRealReady);
        audit_ready_bookmaker(&registry, "fonbet", BookmakerExecutionMode::DryRun);

        let audit = build_execution_state_audit(&registry, replay, 1);

        assert_eq!(audit.total_snapshots, 2);
        assert_eq!(audit.total_transitions, 2);
        assert_eq!(audit.readiness.total_bookmakers, 2);
        assert_eq!(audit.readiness.approval_required, 1);
        assert_eq!(audit.readiness.submit_blocked_by_safe_mode, 1);
        assert_eq!(audit.readiness.sessions_authenticated, 2);
        assert_eq!(audit.readiness.sessions_stale, 0);
        assert_eq!(audit.readiness.balances_stale, 0);
        assert_eq!(audit.readiness.auth_snapshots_stale, 0);
        assert_eq!(audit.bookmaker_readiness.len(), 2);
        assert_eq!(audit.bookmaker_readiness[0].bookmaker, "pari");
        assert!(audit.bookmaker_readiness[0].approval_required);
        assert!(audit.bookmaker_readiness[0].session_authenticated);
        assert!(!audit.bookmaker_readiness[0].session_stale);
        assert!(!audit.bookmaker_readiness[0].balance_stale);
        assert!(!audit.bookmaker_readiness[0].auth_snapshot_stale);
        assert!(audit.bookmaker_readiness[0].persistence_warnings.is_empty());
        assert_eq!(audit.recent_transitions.len(), 1);
        assert_eq!(audit.recent_transitions[0].bookmaker, "pari");
        assert_eq!(audit.recent_transitions[0].to_phase, "Failed");
        assert_eq!(audit.bookmaker_summaries.len(), 2);
        assert_eq!(audit.bookmaker_summaries[0].bookmaker, "pari");
        assert_eq!(audit.bookmaker_summaries[0].phases.failed, 1);
        assert_eq!(
            audit.bookmaker_summaries[0].latest_error.as_deref(),
            Some("coupon rejected")
        );
        assert_eq!(audit.bookmaker_summaries[1].bookmaker, "fonbet");
        assert_eq!(audit.bookmaker_summaries[1].phases.settled, 1);
    }

    #[test]
    fn execution_state_audit_uses_latest_error_timestamp_across_snapshots_and_transitions() {
        let now = Utc::now();
        let placement_id = Uuid::new_v4();
        let registry = ExecutionRegistry::new();
        let replay = ExecutionStateReplay {
            snapshots: vec![
                make_execution_snapshot(
                    placement_id,
                    "pari",
                    ExecutionStatePhase::Failed,
                    BetStatus::Error,
                    1,
                    now - Duration::seconds(30),
                    Some("stale snapshot error"),
                ),
                make_execution_snapshot(
                    placement_id,
                    "pari",
                    ExecutionStatePhase::Failed,
                    BetStatus::Error,
                    2,
                    now - Duration::seconds(5),
                    Some("fresh snapshot error"),
                ),
            ],
            transitions: vec![make_execution_transition(
                placement_id,
                "pari",
                Some(ExecutionStatePhase::ConfirmedPlacement),
                ExecutionStatePhase::Failed,
                BetStatus::Error,
                3,
                now - Duration::seconds(10),
                Some("older transition error"),
            )],
        };

        let audit = build_execution_state_audit(&registry, replay, 5);

        assert_eq!(audit.bookmaker_summaries.len(), 1);
        assert_eq!(
            audit.bookmaker_summaries[0].latest_error.as_deref(),
            Some("fresh snapshot error")
        );
    }

    fn audit_ready_bookmaker(
        registry: &ExecutionRegistry,
        bookmaker: &str,
        mode: BookmakerExecutionMode,
    ) {
        let account_id = Uuid::new_v4();
        registry.register_account(BookmakerAccount {
            id: account_id,
            bookmaker: bookmaker.into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode,
            created_at: Utc::now(),
            last_used_at: None,
        });
        registry.upsert_session(BookmakerSession {
            account_id,
            bookmaker: bookmaker.into(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id,
            bookmaker: bookmaker.into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 7_500.0,
            exposure: 2_500.0,
            captured_at: Utc::now(),
        });
    }

    #[test]
    fn merge_parser_health_reports_open_circuit() {
        let fallback = make_snapshot_health("pari");
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
            last_attempt: Some(Utc::now()),
            last_success: None,
            last_error: Some("boom".into()),
            last_result_status: ParserResultStatus::Failed,
            last_result_message: Some("boom".into()),
            validation_checks: Vec::new(),
            consecutive_failures: 5,
            avg_response_time_ms: 12.0,
            events_parsed: 0,
            odds_parsed: 0,
            uptime_percent: 0.0,
            total_runs: 5,
            successful_runs: 0,
            circuit_state: RuntimeCircuitState::Open,
        };

        let merged = merge_parser_health(&fallback, Some(&runtime));

        assert!(matches!(merged.status, HealthStatus::CircuitOpen));
        assert_eq!(merged.last_error.as_deref(), Some("boom"));
        assert_eq!(merged.consecutive_failures, 5);
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_state"
                && matches!(check.severity, DiagnosticSeverity::Fail)
                && check.message.contains("circuit=open")
        }));
    }

    #[test]
    fn merge_parser_health_marks_stale_runtime_unhealthy() {
        let now = Utc::now();
        let fallback = make_snapshot_health("pari");
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
            last_attempt: Some(now - Duration::seconds(121)),
            last_success: Some(now - Duration::seconds(121)),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 42.0,
            events_parsed: 123,
            odds_parsed: 456,
            uptime_percent: 100.0,
            total_runs: 4,
            successful_runs: 4,
            circuit_state: RuntimeCircuitState::Closed,
        };

        let merged = merge_parser_health_with_freshness(&fallback, Some(&runtime), now, 120);

        assert!(matches!(merged.status, HealthStatus::Unhealthy));
        assert!(merged.diagnostics.iter().any(|check| {
            check.code == "runtime_staleness"
                && matches!(check.severity, DiagnosticSeverity::Fail)
                && check.message.contains("stale_after_secs=120")
        }));
    }

    #[test]
    fn build_live_parsers_health_merges_runtime_over_fallbacks() {
        let fallback_health = vec![ParserHealth {
            bookmaker: "winline".into(),
            status: HealthStatus::Degraded,
            last_success: None,
            last_error: Some(STATIC_PARSER_HEALTH_NOTE.into()),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: Some(ParserReadiness {
                stage: ParserReadinessStage::Production,
                production_enabled: true,
                self_check_available: true,
                checks: vec![ParserDiagnosticCheck {
                    code: "runtime_ready".into(),
                    severity: DiagnosticSeverity::Pass,
                    message: "runtime ready".into(),
                }],
            }),
            diagnostics: vec![ParserDiagnosticCheck {
                code: "runtime_ready".into(),
                severity: DiagnosticSeverity::Pass,
                message: "runtime ready".into(),
            }],
        }];
        let runtime = vec![ParserRuntimeSnapshot {
            bookmaker: "winline".into(),
            last_attempt: Some(Utc::now()),
            last_success: Some(Utc::now()),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 18.0,
            events_parsed: 77,
            odds_parsed: 231,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        }];

        let live = build_live_parsers_health(fallback_health, runtime);
        let winline = live
            .into_iter()
            .find(|item| item.bookmaker == "winline")
            .expect("winline health");

        assert!(matches!(winline.status, HealthStatus::Healthy));
        assert_eq!(winline.events_parsed, 77);
        assert_eq!(winline.avg_response_time_ms, 18.0);
        assert_eq!(winline.last_error, None);
        assert!(winline.readiness.is_some());
        assert!(winline
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_validation"));
        assert!(winline
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_state"));
        assert!(winline
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_throughput"));
        assert!(winline
            .diagnostics
            .iter()
            .all(|check| check.code != "runtime_ready"));
    }

    #[test]
    fn merge_parser_health_keeps_snapshot_context_until_runtime_runs() {
        let fallback = ParserHealth {
            bookmaker: "pari".into(),
            status: HealthStatus::Degraded,
            last_success: None,
            last_error: Some(STATIC_PARSER_HEALTH_NOTE.into()),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: None,
            diagnostics: vec![ParserDiagnosticCheck {
                code: "boot_snapshot".into(),
                severity: DiagnosticSeverity::Info,
                message: "factory snapshot only".into(),
            }],
        };
        let runtime = ParserRuntimeSnapshot {
            bookmaker: "pari".into(),
            last_attempt: None,
            last_success: None,
            last_error: None,
            last_result_status: ParserResultStatus::Failed,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 0.0,
            events_parsed: 0,
            odds_parsed: 0,
            uptime_percent: 0.0,
            total_runs: 0,
            successful_runs: 0,
            circuit_state: RuntimeCircuitState::Closed,
        };

        let merged = merge_parser_health(&fallback, Some(&runtime));

        assert_eq!(
            merged.last_error.as_deref(),
            Some(STATIC_PARSER_HEALTH_NOTE)
        );
        assert!(merged
            .diagnostics
            .iter()
            .any(|check| check.code == "boot_snapshot"));
        assert!(merged
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_state"));
    }

    #[test]
    fn build_live_parsers_health_keeps_runtime_only_parsers() {
        let runtime = vec![ParserRuntimeSnapshot {
            bookmaker: "melbet".into(),
            last_attempt: Some(Utc::now()),
            last_success: Some(Utc::now()),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 22.0,
            events_parsed: 41,
            odds_parsed: 120,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        }];

        let live = build_live_parsers_health(Vec::new(), runtime);
        let melbet = live
            .into_iter()
            .find(|item| item.bookmaker == "melbet")
            .expect("melbet health");

        assert!(matches!(melbet.status, HealthStatus::Healthy));
        assert_eq!(melbet.events_parsed, 41);
        assert!(melbet.readiness.is_none());
        assert_eq!(melbet.last_error, None);
        assert!(melbet
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_state"));
        assert!(melbet
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_throughput"));
    }

    #[test]
    fn build_live_parsers_health_marks_runtime_only_stale_parser() {
        let now = Utc::now();
        let runtime = vec![ParserRuntimeSnapshot {
            bookmaker: "melbet".into(),
            last_attempt: Some(now - Duration::seconds(90)),
            last_success: Some(now - Duration::seconds(90)),
            last_error: None,
            last_result_status: ParserResultStatus::Healthy,
            last_result_message: None,
            validation_checks: Vec::new(),
            consecutive_failures: 0,
            avg_response_time_ms: 22.0,
            events_parsed: 41,
            odds_parsed: 120,
            uptime_percent: 100.0,
            total_runs: 1,
            successful_runs: 1,
            circuit_state: RuntimeCircuitState::Closed,
        }];

        let live = build_live_parsers_health_with_freshness(Vec::new(), runtime, now, 60);
        let melbet = live
            .into_iter()
            .find(|item| item.bookmaker == "melbet")
            .expect("melbet health");

        assert!(matches!(melbet.status, HealthStatus::Unhealthy));
        assert!(melbet
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_staleness"));
    }

    #[test]
    fn build_live_parsers_coverage_attaches_runtime_health() {
        let fallback_coverage = vec![ParserCoverage {
            slug: "ligastavok".into(),
            name: "Liga Stavok".into(),
            enabled: false,
            scan_supported: false,
            execution_supported: false,
            status: shared::BookmakerStatus::Disabled,
            parser_type: "api".into(),
            source: "crates/parsers/src/ligastavok.rs".into(),
            notes: Some("disabled for diagnostics".into()),
            readiness: Some(ParserReadiness {
                stage: ParserReadinessStage::DiagnosticOnly,
                production_enabled: false,
                self_check_available: true,
                checks: vec![ParserDiagnosticCheck {
                    code: "qrator_unattended_bootstrap_unverified".into(),
                    severity: DiagnosticSeverity::Warn,
                    message: "bootstrap is unverified".into(),
                }],
            }),
            runtime_health: None,
        }];
        let live_health = vec![ParserHealth {
            bookmaker: "ligastavok".into(),
            status: HealthStatus::CircuitOpen,
            last_success: None,
            last_error: Some("runtime failure".into()),
            consecutive_failures: 5,
            avg_response_time_ms: 31.0,
            events_parsed: 0,
            uptime_percent: 0.0,
            readiness: None,
            diagnostics: vec![ParserDiagnosticCheck {
                code: "runtime_state".into(),
                severity: DiagnosticSeverity::Fail,
                message: "runtime circuit=open".into(),
            }],
        }];

        let live = build_live_parsers_coverage(fallback_coverage, live_health);
        let ligastavok = live
            .into_iter()
            .find(|item| item.slug == "ligastavok")
            .expect("ligastavok coverage");

        assert!(ligastavok.readiness.is_some());
        assert!(matches!(
            ligastavok
                .runtime_health
                .as_ref()
                .expect("runtime health")
                .status,
            HealthStatus::CircuitOpen
        ));
        let runtime_health = ligastavok.runtime_health.expect("runtime health");
        assert!(runtime_health
            .diagnostics
            .iter()
            .any(|check| check.code == "runtime_state"));
    }

    #[test]
    fn build_live_parsers_coverage_keeps_runtime_only_slugs() {
        let live = build_live_parsers_coverage(
            Vec::new(),
            vec![ParserHealth {
                bookmaker: "runtime-book".into(),
                status: HealthStatus::Healthy,
                last_success: Some(Utc::now()),
                last_error: None,
                consecutive_failures: 0,
                avg_response_time_ms: 14.0,
                events_parsed: 12,
                uptime_percent: 100.0,
                readiness: None,
                diagnostics: Vec::new(),
            }],
        );

        assert_eq!(live.len(), 1);
        assert_eq!(live[0].slug, "runtime-book");
        assert_eq!(live[0].name, "runtime-book");
        assert!(live[0].enabled);
        assert!(live[0].scan_supported);
        assert_eq!(live[0].source, "runtime");
        assert!(matches!(live[0].status, shared::BookmakerStatus::ScanOnly));
        assert!(matches!(
            live[0].runtime_health.as_ref().map(|item| &item.status),
            Some(HealthStatus::Healthy)
        ));
    }

    fn make_bet_placement(status: BetStatus) -> BetPlacement {
        BetPlacement {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
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
            odds: 2.0,
            stake: 500.0,
            status,
            placed_at: Utc::now(),
            execution: None,
            result: None,
            error: None,
        }
    }

    fn make_ledger_entry(
        action: auto_betting::ExecutionLedgerAction,
        status: BetStatus,
    ) -> auto_betting::ExecutionLedgerEntry {
        auto_betting::ExecutionLedgerEntry {
            placement: make_bet_placement(status),
            action,
            recorded_at: Utc::now(),
        }
    }

    fn make_state_replay() -> ExecutionStateReplay {
        let placed = make_ledger_entry(
            auto_betting::ExecutionLedgerAction::Placed,
            BetStatus::Pending,
        );
        let mut settled = make_ledger_entry(
            auto_betting::ExecutionLedgerAction::Updated,
            BetStatus::Settled,
        );
        settled.placement.id = placed.placement.id;
        settled.recorded_at = placed.recorded_at + chrono::Duration::seconds(30);
        auto_betting::ExecutionStateMachine::replay([&placed, &settled]).expect("valid replay")
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
        assert!((plan.required_cash_by_bookmaker["pari"] - 238.09523809523807).abs() < 1e-9);
        assert!((plan.required_cash_by_bookmaker["fonbet"] - 1_523.8095238095236).abs() < 1e-9);
        assert!(plan.funding_recommendation.contains("pari: 238.10"));
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
        assert_eq!(summary.accounts_with_control_issues, 0);
        assert_eq!(summary.sessions_authenticated, 1);
        assert_eq!(summary.sessions_stale, 0);
        assert_eq!(summary.balances_cached, 1);
        assert_eq!(summary.balances_stale, 0);
        assert_eq!(summary.auth_snapshots_stale, 0);
        assert_eq!(summary.ready_for_dry_run, 1);
        assert_eq!(summary.ready_for_execution, 1);
    }

    #[test]
    fn account_summary_counts_stale_persisted_readiness_snapshots() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "fonbet".into(),
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
            last_synced_at: Utc::now() - Duration::minutes(20),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: account.id,
            bookmaker: account.bookmaker.clone(),
            currency: account.currency.clone(),
            total_balance: 10_000.0,
            available_balance: 8_500.0,
            exposure: 1_500.0,
            captured_at: Utc::now() - Duration::minutes(7),
        });

        let summary = build_account_session_summary(&registry);
        let state = get_account_state(&registry, "fonbet").expect("account state should exist");

        assert_eq!(summary.sessions_stale, 1);
        assert_eq!(summary.balances_stale, 1);
        assert_eq!(summary.auth_snapshots_stale, 0);
        assert!(state.persistence_status.session_stale);
        assert!(state.persistence_status.balance_stale);
        assert!(!state.persistence_status.auth_snapshot_stale);
        assert!(state
            .persistence_status
            .warnings
            .iter()
            .any(|warning| warning.contains("session snapshot is stale")));
        assert!(state
            .persistence_status
            .warnings
            .iter()
            .any(|warning| warning.contains("balance snapshot is stale")));
    }

    #[test]
    fn persistence_status_marks_auth_snapshot_behind_newer_persisted_state() {
        let now = Utc::now();
        let account_id = Uuid::new_v4();
        let session = BookmakerSession {
            account_id,
            bookmaker: "pari".into(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: now - Duration::minutes(2),
            expires_at: Some(now + Duration::minutes(10)),
        };
        let balance = BookmakerBalanceSnapshot {
            account_id,
            bookmaker: "pari".into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 9_000.0,
            exposure: 1_000.0,
            captured_at: now - Duration::minutes(1),
        };
        let auth_snapshot = BookmakerAuthSnapshot {
            account_id: Some(account_id),
            bookmaker: "pari".into(),
            auth_state: shared::BookmakerAuthState::Authenticated,
            readiness_stage: shared::BookmakerAdapterReadinessStage::AuthenticatedReadOnly,
            mode: Some(BookmakerExecutionMode::DryRun),
            enabled: true,
            cached_balance_available: true,
            submit_enabled: false,
            real_money_enabled: false,
            safe_mode_blocked: false,
            session_last_synced_at: Some(session.last_synced_at),
            balance_captured_at: Some(balance.captured_at),
            last_authenticated_at: Some(session.last_synced_at),
            detail: Some("persisted auth snapshot".into()),
            captured_at: now - Duration::minutes(3),
        };

        let status = build_account_persistence_status(
            "pari",
            Some(&session),
            Some(&balance),
            Some(&auth_snapshot),
            now,
        );

        assert!(!status.session_stale);
        assert!(!status.balance_stale);
        assert!(status.auth_snapshot_stale);
        assert!(status
            .warnings
            .iter()
            .any(|warning| warning.contains("lags behind newer persisted session/balance state")));
    }

    #[test]
    fn bankroll_sync_applies_refresh_snapshot() {
        let bankroll_manager = BankrollManager::new(shared::BankrollConfig::default());
        let account_id = Uuid::new_v4();
        let refresh = BookmakerBalanceRefresh {
            account_id: Some(account_id),
            bookmaker: "pari".into(),
            state: BookmakerBalanceRefreshState::CachedBalanceAvailable,
            session_status: shared::BookmakerSessionStatus {
                account_id: Some(account_id),
                bookmaker: "pari".into(),
                sync_state: shared::BookmakerSessionSyncState::Authenticated,
                authenticated: true,
                can_refresh_balance: true,
                detail: None,
                checked_at: Utc::now(),
            },
            snapshot: Some(BookmakerBalanceSnapshot {
                account_id,
                bookmaker: "pari".into(),
                currency: "RUB".into(),
                total_balance: 12_000.0,
                available_balance: 10_500.0,
                exposure: 1_500.0,
                captured_at: Utc::now(),
            }),
            detail: None,
            checked_at: Utc::now(),
        };

        sync_bankroll_with_balance_snapshot(&bankroll_manager, refresh.snapshot.as_ref());

        let state = bankroll_manager.get_state();
        assert_eq!(state.bookmakers.len(), 1);
        assert_eq!(state.bookmakers[0].bookmaker, "pari");
        assert_eq!(state.bookmakers[0].balance, 12_000.0);
        assert_eq!(state.bookmakers[0].available, 10_500.0);
    }

    #[test]
    fn bankroll_sync_imports_cached_registry_snapshots() {
        let registry = ExecutionRegistry::new();
        let bankroll_manager = BankrollManager::new(shared::BankrollConfig::default());
        let pari_account_id = Uuid::new_v4();
        let fonbet_account_id = Uuid::new_v4();

        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: pari_account_id,
            bookmaker: "pari".into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 8_000.0,
            exposure: 2_000.0,
            captured_at: Utc::now(),
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id: fonbet_account_id,
            bookmaker: "fonbet".into(),
            currency: "RUB".into(),
            total_balance: 9_000.0,
            available_balance: 8_500.0,
            exposure: 500.0,
            captured_at: Utc::now(),
        });

        sync_bankroll_with_registry_snapshots(&registry, &bankroll_manager);

        let state = bankroll_manager.get_state();
        assert_eq!(state.bookmakers.len(), 2);
        assert_eq!(state.total_exposure, 2_500.0);
        assert!(state.bookmakers.iter().any(|item| item.bookmaker == "pari"));
        assert!(state
            .bookmakers
            .iter()
            .any(|item| item.bookmaker == "fonbet"));
    }

    #[test]
    fn account_state_surfaces_control_issues_for_operator_audit() {
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

        let state = get_account_state(&registry, "pari").expect("account state should exist");

        assert_eq!(state.control_issues.len(), 2);
        assert!(!state.readiness.session_ready);
        assert!(!state.readiness.balance_ready);
        assert!(!state.readiness.dry_run_ready);
        assert!(!state.readiness.can_arm_safely);
        assert!(state.readiness.rollout_gate_active);
        assert!(!state.readiness.approval_required);
        assert!(!state.readiness.submit_blocked_by_safe_mode);
        assert!(state
            .readiness
            .operator_action
            .as_deref()
            .unwrap_or_default()
            .contains("refresh the pari session"));
        assert!(state
            .readiness
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("rollout gate")));
        assert!(state
            .control_issues
            .iter()
            .any(|issue| issue.contains("active session")));
        assert!(state
            .control_issues
            .iter()
            .any(|issue| issue.contains("cached balance snapshot")));
        assert_eq!(
            state
                .auth_snapshot
                .as_ref()
                .expect("auth snapshot should exist")
                .detail
                .as_deref(),
            Some("pari adapter still requires operator-managed session/bootstrap readiness")
        );
    }

    #[test]
    fn account_summary_counts_accounts_with_control_issues() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: false,
            mode: BookmakerExecutionMode::Armed,
            created_at: Utc::now(),
            last_used_at: None,
        };

        registry.register_account(account);

        let summary = build_account_session_summary(&registry);

        assert_eq!(summary.accounts_with_control_issues, 1);
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
                confirm_rollout_gate_acknowledged: None,
            },
        )
        .expect_err("missing confirmation must be rejected");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("confirm_dry_run_only"));
    }

    #[test]
    fn account_control_update_rejects_arming_without_readiness() {
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
                confirm_dry_run_only: true,
                confirm_rollout_gate_acknowledged: Some(true),
            },
        )
        .expect_err("arming without session/balance should be rejected");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("active bookmaker session") || error.1.contains("balance"));
    }

    #[test]
    fn execution_placements_fall_back_to_ledger_when_runtime_history_is_empty() {
        let placement = make_bet_placement(BetStatus::Settled);
        let placements = merge_execution_placements(Vec::new(), vec![placement.clone()]);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].id, placement.id);
        assert_eq!(placements[0].status, BetStatus::Settled);
    }

    #[test]
    fn execution_placements_prefer_runtime_history_when_available() {
        let runtime = make_bet_placement(BetStatus::Placed);
        let ledger = make_bet_placement(BetStatus::Settled);
        let placements = merge_execution_placements(vec![runtime.clone()], vec![ledger]);

        assert_eq!(placements.len(), 2);
        assert_eq!(placements[0].id, runtime.id);
        assert_eq!(placements[0].status, BetStatus::Placed);
    }

    #[test]
    fn execution_placements_deduplicate_ledger_entries_for_runtime_bets() {
        let runtime = make_bet_placement(BetStatus::Placed);
        let mut ledger = runtime.clone();
        ledger.status = BetStatus::Settled;

        let placements = merge_execution_placements(vec![runtime.clone()], vec![ledger]);

        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].id, runtime.id);
        assert_eq!(placements[0].status, BetStatus::Placed);
    }

    #[test]
    fn execution_ledger_audit_reports_recent_records_and_unique_placements() {
        let placed = make_ledger_entry(
            auto_betting::ExecutionLedgerAction::Placed,
            BetStatus::Placed,
        );
        let mut updated = make_ledger_entry(
            auto_betting::ExecutionLedgerAction::Updated,
            BetStatus::Settled,
        );
        updated.placement.id = placed.placement.id;
        updated.recorded_at = placed.recorded_at + chrono::Duration::seconds(30);

        let audit = build_execution_ledger_audit(
            vec![updated.clone(), placed.clone()],
            build_execution_state_metadata(make_state_replay(), 5),
        );

        assert_eq!(audit.total_entries, 2);
        assert_eq!(audit.unique_placements, 1);
        assert_eq!(audit.latest_recorded_at, Some(updated.recorded_at));
        assert_eq!(audit.state_machine.total_snapshots, 1);
        assert_eq!(audit.state_machine.total_transitions, 2);
        assert_eq!(audit.recent_records.len(), 2);
        assert_eq!(audit.recent_records[0].action, "Updated");
        assert_eq!(audit.recent_records[0].placement.status, BetStatus::Settled);
    }

    #[test]
    fn execution_state_metadata_summarizes_phase_counts_and_recent_snapshots() {
        let metadata = build_execution_state_metadata(make_state_replay(), 1);

        assert_eq!(metadata.total_snapshots, 1);
        assert_eq!(metadata.total_transitions, 2);
        assert_eq!(metadata.phases.settled, 1);
        assert_eq!(metadata.recent_snapshots.len(), 1);
        assert_eq!(metadata.recent_snapshots[0].phase, "Settled");
        assert_eq!(metadata.recent_snapshots[0].sequence, 2);
        assert_eq!(metadata.recent_snapshots[0].last_action, "Updated");
        assert!(metadata.latest_snapshot_at.is_some());
        assert!(metadata.latest_transition_at.is_some());
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
                confirm_rollout_gate_acknowledged: None,
            },
        )
        .expect("safe downgrade should succeed");

        let account = updated.account.expect("account payload should be present");
        assert!(account.enabled);
        assert_eq!(account.mode, BookmakerExecutionMode::DryRun);
    }

    #[test]
    fn account_control_update_requires_rollout_gate_ack_for_pari_arming() {
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
                confirm_dry_run_only: true,
                confirm_rollout_gate_acknowledged: None,
            },
        )
        .expect_err("pari rollout arming must require explicit acknowledgement");

        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("confirm_rollout_gate_acknowledged"));
    }

    #[test]
    fn account_state_marks_pari_submission_as_safe_mode_only() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::SemiRealReady,
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

        let state = get_account_state(&registry, "pari").expect("account state should exist");

        assert!(state.readiness.session_ready);
        assert!(state.readiness.balance_ready);
        assert!(state.readiness.placement_ready);
        assert!(state.readiness.rollout_gate_active);
        assert!(state.readiness.approval_required);
        assert!(state.readiness.submit_blocked_by_safe_mode);
        assert!(state
            .readiness
            .operator_action
            .as_deref()
            .unwrap_or_default()
            .contains("coupon submit remains disabled"));
        let auth_snapshot = state.auth_snapshot.expect("auth snapshot should exist");
        assert!(auth_snapshot.safe_mode_blocked);
        assert_eq!(
            auth_snapshot.readiness_stage,
            shared::BookmakerAdapterReadinessStage::SafeModePlacementReady
        );
    }

    #[test]
    fn account_state_marks_expired_fonbet_session_as_not_ready() {
        let registry = ExecutionRegistry::new();
        let account = BookmakerAccount {
            id: Uuid::new_v4(),
            bookmaker: "fonbet".into(),
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
            expires_at: Some(Utc::now() - Duration::minutes(5)),
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

        let state = get_account_state(&registry, "fonbet").expect("account state should exist");

        assert!(!state.readiness.session_ready);
        assert!(!state.readiness.dry_run_ready);
        assert!(state
            .readiness
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("expiry timestamp has passed")));
        assert!(state
            .readiness
            .operator_action
            .as_deref()
            .unwrap_or_default()
            .contains("refresh the fonbet session"));
        assert!(state
            .control_issues
            .iter()
            .any(|issue| issue.contains("locally expired session timestamp")));
        assert_eq!(
            state
                .auth_snapshot
                .as_ref()
                .expect("auth snapshot should exist")
                .auth_state,
            shared::BookmakerAuthState::Expired
        );
    }

    #[test]
    fn execution_state_readiness_does_not_count_expired_sessions_as_authenticated() {
        let registry = ExecutionRegistry::new();
        let account_id = Uuid::new_v4();

        registry.register_account(BookmakerAccount {
            id: account_id,
            bookmaker: "fonbet".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        });
        registry.upsert_session(BookmakerSession {
            account_id,
            bookmaker: "fonbet".into(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: Some(Utc::now() - Duration::minutes(5)),
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id,
            bookmaker: "fonbet".into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 8_000.0,
            exposure: 2_000.0,
            captured_at: Utc::now(),
        });

        let (summary, bookmaker_readiness) = build_execution_state_readiness(&registry);
        let fonbet = bookmaker_readiness
            .iter()
            .find(|item| item.bookmaker == "fonbet")
            .expect("fonbet readiness");

        assert_eq!(summary.sessions_authenticated, 0);
        assert_eq!(summary.sessions_stale, 1);
        assert!(!fonbet.session_authenticated);
        assert!(!fonbet.auth_ready);
        assert!(fonbet.session_stale);
        assert!(!fonbet.balance_stale);
        assert!(!fonbet.auth_snapshot_stale);
        assert!(fonbet
            .persistence_warnings
            .iter()
            .any(|warning| warning.contains("persisted session snapshot is stale")));
    }

    #[test]
    fn execution_state_readiness_surfaces_stale_balance_snapshots() {
        let registry = ExecutionRegistry::new();
        let account_id = Uuid::new_v4();

        registry.register_account(BookmakerAccount {
            id: account_id,
            bookmaker: "pari".into(),
            label: "main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        });
        registry.upsert_session(BookmakerSession {
            account_id,
            bookmaker: "pari".into(),
            state: BookmakerSessionState::Active,
            token_hint: Some("sess...".into()),
            last_synced_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::minutes(30)),
        });
        registry.upsert_balance_snapshot(BookmakerBalanceSnapshot {
            account_id,
            bookmaker: "pari".into(),
            currency: "RUB".into(),
            total_balance: 10_000.0,
            available_balance: 8_000.0,
            exposure: 2_000.0,
            captured_at: Utc::now() - Duration::minutes(10),
        });

        let (summary, bookmaker_readiness) = build_execution_state_readiness(&registry);
        let pari = bookmaker_readiness
            .iter()
            .find(|item| item.bookmaker == "pari")
            .expect("pari readiness");

        assert_eq!(summary.balances_stale, 1);
        assert!(pari.balance_cached);
        assert!(pari.balance_stale);
        assert!(!pari.session_stale);
        assert!(!pari.auth_snapshot_stale);
        assert!(pari
            .persistence_warnings
            .iter()
            .any(|warning| warning.contains("persisted balance snapshot is stale")));
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
            required_cash_by_bookmaker: HashMap::from([
                ("pari".into(), 500.0),
                ("fonbet".into(), 400.0),
            ]),
            funding_recommendation:
                "Keep cash ready before starting the sequence: fonbet: 400.00, pari: 500.00. The 1000.00 freebet itself is placed at pari without extra cash stake.".into(),
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
                next_milestone: "close_funding_gap".into(),
                blocked_by: vec!["funding:pari".into()],
                read_only_follow_up:
                    "After balances update, refresh lifecycle tracking and confirm the draft leaves awaiting_funding."
                        .into(),
                read_only_focus: "balance_refresh".into(),
                opportunity: Some(opportunity),
                bonus: Some(make_freebet_bonus(BonusStatus::Claimed, 0.0)),
                plan: Some(plan),
                rollover: None,
                allocation: Some(shared::FreebetBookmakerAllocation {
                    bookmaker: "pari".into(),
                    available_balance: Some(500.0),
                    recommended_deposit: Some(250.0),
                    deposit_gap: Some(250.0),
                    urgency: Some(shared::DepositUrgency::Medium),
                    note: "top up".into(),
                }),
                auto_rollover: Some(FreebetAutoRolloverDraft {
                    status: FreebetAutoRolloverStatus::AwaitingFunding,
                    safe_mode: true,
                    execution_allowed: false,
                    required_cash_by_bookmaker: HashMap::from([
                        ("pari".into(), 500.0),
                        ("fonbet".into(), 400.0),
                    ]),
                    funding_gap_by_bookmaker: HashMap::from([("pari".into(), 250.0)]),
                    funding_readiness: FreebetFundingReadiness {
                        ready: false,
                        total_gap: 250.0,
                        blocking_bookmakers: vec!["pari".into()],
                        largest_gap_bookmaker: Some("pari".into()),
                        largest_gap_amount: Some(250.0),
                    },
                    funding_recommendation:
                        "Keep cash ready before starting the sequence: fonbet: 400.00, pari: 500.00. The 1000.00 freebet itself is placed at pari without extra cash stake.".into(),
                    trigger:
                        "funding gaps must be closed before rollover draft can start".into(),
                    next_action: "Top up pari by at least 250.00 before reviewing the draft again."
                        .into(),
                    read_only_check:
                        "After balances update, refresh lifecycle tracking and confirm the draft leaves awaiting_funding."
                            .into(),
                    notes: vec!["draft only".into()],
                }),
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
            FreebetLifecycleState {
                bookmaker: "fonbet".into(),
                lifecycle_stage: FreebetLifecycleStage::RolloverCompleted,
                next_milestone: "audit_snapshot".into(),
                blocked_by: Vec::new(),
                read_only_follow_up:
                    "Refresh lifecycle only for audit and confirm the snapshot stays completed."
                        .into(),
                read_only_focus: "completion_audit".into(),
                opportunity: None,
                bonus: None,
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: None,
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
        ];

        let summary = build_freebet_lifecycle_summary(&states);

        assert_eq!(summary.total_bookmakers, 2);
        assert_eq!(summary.opportunities, 1);
        assert_eq!(summary.active_bonuses, 1);
        assert_eq!(summary.tracked_plans, 1);
        assert_eq!(summary.deposit_required_bookmakers, 1);
        assert_eq!(summary.blocked_states, 1);
        assert_eq!(summary.total_funding_gap, 250.0);
        assert_eq!(
            summary
                .largest_funding_gap
                .as_ref()
                .map(|item| (item.bookmaker.as_str(), item.amount)),
            Some(("pari", 250.0))
        );
        assert_eq!(summary.planned, 1);
        assert_eq!(summary.rollover_completed, 1);
        assert_eq!(summary.next_milestones.len(), 2);
        assert_eq!(summary.next_milestones[0].label, "audit_snapshot");
        assert_eq!(summary.next_milestones[0].count, 1);
        assert_eq!(summary.next_milestones[1].label, "close_funding_gap");
        assert_eq!(summary.blockers.len(), 1);
        assert_eq!(summary.blockers[0].label, "funding:pari");
        assert_eq!(summary.blockers[0].count, 1);
        assert_eq!(summary.read_only_focuses.len(), 2);
        assert_eq!(summary.read_only_focuses[0].label, "balance_refresh");
        assert_eq!(summary.read_only_focuses[0].count, 1);
        assert_eq!(summary.read_only_focuses[1].label, "completion_audit");
        assert_eq!(summary.total_freebet_amount, 1_000.0);
        assert_eq!(summary.total_estimated_profit, 120.0);
    }

    #[test]
    fn freebet_summary_accumulates_funding_gap_across_drafts() {
        let states = vec![
            FreebetLifecycleState {
                bookmaker: "pari".into(),
                lifecycle_stage: FreebetLifecycleStage::Planned,
                next_milestone: "close_funding_gap".into(),
                blocked_by: vec!["funding:pari".into()],
                read_only_follow_up: String::new(),
                read_only_focus: "balance_refresh".into(),
                opportunity: None,
                bonus: None,
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: Some(FreebetAutoRolloverDraft {
                    status: FreebetAutoRolloverStatus::AwaitingFunding,
                    safe_mode: true,
                    execution_allowed: false,
                    required_cash_by_bookmaker: HashMap::new(),
                    funding_gap_by_bookmaker: HashMap::from([("pari".into(), 125.0)]),
                    funding_readiness: FreebetFundingReadiness {
                        ready: false,
                        total_gap: 125.0,
                        blocking_bookmakers: vec!["pari".into()],
                        largest_gap_bookmaker: Some("pari".into()),
                        largest_gap_amount: Some(125.0),
                    },
                    funding_recommendation: String::new(),
                    trigger: String::new(),
                    next_action: String::new(),
                    read_only_check: String::new(),
                    notes: Vec::new(),
                }),
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
            FreebetLifecycleState {
                bookmaker: "fonbet".into(),
                lifecycle_stage: FreebetLifecycleStage::Qualified,
                next_milestone: "prepare_conversion_plan".into(),
                blocked_by: vec!["funding:fonbet".into()],
                read_only_follow_up: String::new(),
                read_only_focus: "draft_review".into(),
                opportunity: None,
                bonus: None,
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: Some(FreebetAutoRolloverDraft {
                    status: FreebetAutoRolloverStatus::AwaitingFunding,
                    safe_mode: true,
                    execution_allowed: false,
                    required_cash_by_bookmaker: HashMap::new(),
                    funding_gap_by_bookmaker: HashMap::from([("fonbet".into(), 75.0)]),
                    funding_readiness: FreebetFundingReadiness {
                        ready: false,
                        total_gap: 75.0,
                        blocking_bookmakers: vec!["fonbet".into()],
                        largest_gap_bookmaker: Some("fonbet".into()),
                        largest_gap_amount: Some(75.0),
                    },
                    funding_recommendation: String::new(),
                    trigger: String::new(),
                    next_action: String::new(),
                    read_only_check: String::new(),
                    notes: Vec::new(),
                }),
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
            FreebetLifecycleState {
                bookmaker: "winline".into(),
                lifecycle_stage: FreebetLifecycleStage::Discovered,
                next_milestone: "review_opportunity".into(),
                blocked_by: Vec::new(),
                read_only_follow_up: String::new(),
                read_only_focus: "odds_sync".into(),
                opportunity: None,
                bonus: None,
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: None,
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
        ];

        let summary = build_freebet_lifecycle_summary(&states);

        assert_eq!(summary.total_funding_gap, 200.0);
        assert_eq!(
            summary
                .largest_funding_gap
                .as_ref()
                .map(|item| (item.bookmaker.as_str(), item.amount)),
            Some(("pari", 125.0))
        );
    }

    #[test]
    fn freebet_summary_leaves_largest_funding_gap_empty_without_drafts() {
        let summary = build_freebet_lifecycle_summary(&[FreebetLifecycleState {
            bookmaker: "winline".into(),
            lifecycle_stage: FreebetLifecycleStage::Discovered,
            next_milestone: "review_opportunity".into(),
            blocked_by: Vec::new(),
            read_only_follow_up: String::new(),
            read_only_focus: "odds_sync".into(),
            opportunity: None,
            bonus: None,
            plan: None,
            rollover: None,
            allocation: None,
            auto_rollover: None,
            rollover_actions: Vec::new(),
            execution_readiness: None,
            updated_at: Utc::now(),
        }]);

        assert_eq!(summary.total_funding_gap, 0.0);
        assert!(summary.largest_funding_gap.is_none());
    }

    #[tokio::test]
    async fn freebet_lifecycle_store_returns_persisted_snapshot() {
        let store = FreebetLifecycleStore::new("memory").await.unwrap();
        let persisted = FreebetLifecycleState {
            bookmaker: "pari".into(),
            lifecycle_stage: FreebetLifecycleStage::Planned,
            next_milestone: "place_manual_legs".into(),
            blocked_by: vec!["manual_trigger".into()],
            read_only_follow_up:
                "After the manual legs settle, refresh lifecycle tracking and confirm the draft enters monitoring."
                    .into(),
            read_only_focus: "manual_settlement".into(),
            opportunity: Some(make_freebet_opportunity()),
            bonus: Some(make_freebet_bonus(BonusStatus::Claimed, 0.0)),
            plan: None,
            rollover: None,
            allocation: None,
            auto_rollover: Some(FreebetAutoRolloverDraft {
                status: FreebetAutoRolloverStatus::AwaitingTrigger,
                safe_mode: true,
                execution_allowed: false,
                required_cash_by_bookmaker: HashMap::new(),
                funding_gap_by_bookmaker: HashMap::new(),
                funding_readiness: FreebetFundingReadiness {
                    ready: true,
                    total_gap: 0.0,
                    blocking_bookmakers: Vec::new(),
                    largest_gap_bookmaker: None,
                    largest_gap_amount: None,
                },
                funding_recommendation: "no funding payload available yet".into(),
                trigger: "wait for manual qualifying/freebet placement; draft stays in no-op mode"
                    .into(),
                next_action:
                    "Place the qualifying/freebet legs manually, then refresh lifecycle tracking."
                        .into(),
                read_only_check:
                    "After the manual legs settle, refresh lifecycle tracking and confirm the draft enters monitoring."
                        .into(),
                notes: vec!["draft only".into()],
            }),
            rollover_actions: Vec::new(),
            execution_readiness: None,
            updated_at: Utc::now(),
        };

        store.save_state(&persisted).await.unwrap();

        let selected = select_freebet_lifecycle_snapshot(Vec::new(), store.list_states().await);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].bookmaker, "pari");
        assert_eq!(selected[0].lifecycle_stage, FreebetLifecycleStage::Planned);
    }

    #[test]
    fn freebet_lifecycle_merges_live_and_persisted_snapshots() {
        let live = vec![FreebetLifecycleState {
            bookmaker: "pari".into(),
            lifecycle_stage: FreebetLifecycleStage::Discovered,
            next_milestone: String::new(),
            blocked_by: Vec::new(),
            read_only_follow_up: String::new(),
            read_only_focus: String::new(),
            opportunity: Some(make_freebet_opportunity()),
            bonus: None,
            plan: None,
            rollover: None,
            allocation: None,
            auto_rollover: None,
            rollover_actions: Vec::new(),
            execution_readiness: None,
            updated_at: Utc::now(),
        }];
        let persisted = vec![
            FreebetLifecycleState {
                bookmaker: "pari".into(),
                lifecycle_stage: FreebetLifecycleStage::Planned,
                next_milestone: "place_manual_legs".into(),
                blocked_by: vec!["manual_trigger".into()],
                read_only_follow_up:
                    "Refresh lifecycle after manual placement and confirm the draft is still aligned."
                        .into(),
                read_only_focus: "manual_placement".into(),
                opportunity: None,
                bonus: Some(make_freebet_bonus(BonusStatus::Claimed, 0.0)),
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: None,
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now() - Duration::seconds(10),
            },
            FreebetLifecycleState {
                bookmaker: "fonbet".into(),
                lifecycle_stage: FreebetLifecycleStage::Qualified,
                next_milestone: "prepare_conversion_plan".into(),
                blocked_by: vec!["manual_trigger".into()],
                read_only_follow_up: "persisted follow-up".into(),
                read_only_focus: "manual_placement".into(),
                opportunity: None,
                bonus: Some(make_freebet_bonus(BonusStatus::Claimed, 0.0)),
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: None,
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
        ];

        let selected = select_freebet_lifecycle_snapshot(live, persisted);

        assert_eq!(selected.len(), 2);
        let fonbet = selected
            .iter()
            .find(|item| item.bookmaker == "fonbet")
            .expect("persisted bookmaker");
        assert_eq!(fonbet.lifecycle_stage, FreebetLifecycleStage::Qualified);

        let pari = selected
            .iter()
            .find(|item| item.bookmaker == "pari")
            .expect("merged bookmaker");
        assert_eq!(pari.lifecycle_stage, FreebetLifecycleStage::Discovered);
        assert!(pari.opportunity.is_some());
        assert!(pari.bonus.is_some());
        assert_eq!(pari.next_milestone, "place_manual_legs");
        assert_eq!(pari.read_only_focus, "manual_placement");
        assert_eq!(pari.blocked_by, vec!["manual_trigger"]);
    }

    #[test]
    fn enriches_freebet_lifecycle_with_execution_bridge_metadata() {
        let registry = ExecutionRegistry::new();
        let account_id = Uuid::new_v4();
        registry.register_account(BookmakerAccount {
            id: account_id,
            bookmaker: "pari".into(),
            label: "Pari main".into(),
            currency: "RUB".into(),
            enabled: true,
            mode: BookmakerExecutionMode::DryRun,
            created_at: Utc::now(),
            last_used_at: None,
        });
        registry.upsert_session(BookmakerSession {
            account_id,
            bookmaker: "pari".into(),
            state: BookmakerSessionState::Active,
            token_hint: Some("cached".into()),
            last_synced_at: Utc::now(),
            expires_at: None,
        });
        registry.upsert_balance_snapshot(shared::BookmakerBalanceSnapshot {
            account_id,
            bookmaker: "pari".into(),
            currency: "RUB".into(),
            total_balance: 2_500.0,
            available_balance: 2_100.0,
            exposure: 0.0,
            captured_at: Utc::now(),
        });

        let enriched = enrich_freebet_lifecycle_state(
            &registry,
            FreebetLifecycleState {
                bookmaker: "pari".into(),
                lifecycle_stage: FreebetLifecycleStage::Planned,
                next_milestone: "place_manual_legs".into(),
                blocked_by: vec!["manual_trigger".into()],
                read_only_follow_up: String::new(),
                read_only_focus: "manual_settlement".into(),
                opportunity: Some(make_freebet_opportunity()),
                bonus: Some(make_freebet_bonus(BonusStatus::Claimed, 0.0)),
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: Some(FreebetAutoRolloverDraft {
                    status: FreebetAutoRolloverStatus::AwaitingTrigger,
                    safe_mode: true,
                    execution_allowed: false,
                    required_cash_by_bookmaker: HashMap::new(),
                    funding_gap_by_bookmaker: HashMap::new(),
                    funding_readiness: FreebetFundingReadiness {
                        ready: true,
                        total_gap: 0.0,
                        blocking_bookmakers: Vec::new(),
                        largest_gap_bookmaker: None,
                        largest_gap_amount: None,
                    },
                    funding_recommendation: "balances already cover the draft".into(),
                    trigger:
                        "wait for manual qualifying/freebet placement; draft stays in no-op mode"
                            .into(),
                    next_action: String::new(),
                    read_only_check: String::new(),
                    notes: Vec::new(),
                }),
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
        );

        assert_eq!(enriched.rollover_actions.len(), 4);
        assert_eq!(enriched.rollover_actions[1].key, "manual_trigger");
        assert_eq!(
            enriched
                .execution_readiness
                .as_ref()
                .map(|item| &item.stage),
            Some(&FreebetExecutionReadinessStage::AwaitingManualTrigger)
        );
        let readiness = enriched.execution_readiness.as_ref().unwrap();
        assert!(readiness.account_configured);
        assert!(readiness.session_ready);
        assert!(readiness.balance_snapshot_available);
        assert!(!readiness.dry_run_ready);
        assert!(readiness.manual_trigger_required);
        assert!(readiness
            .blocking_reasons
            .iter()
            .any(|item| item.contains("manual qualifying/freebet trigger")));
        assert!(!readiness.real_money_enabled);
    }

    #[test]
    fn execution_bridge_marks_funding_blocked_before_dry_run() {
        let registry = ExecutionRegistry::new();

        let enriched = enrich_freebet_lifecycle_state(
            &registry,
            FreebetLifecycleState {
                bookmaker: "pari".into(),
                lifecycle_stage: FreebetLifecycleStage::Planned,
                next_milestone: "close_funding_gap".into(),
                blocked_by: vec!["funding:pari".into()],
                read_only_follow_up: String::new(),
                read_only_focus: "balance_refresh".into(),
                opportunity: Some(make_freebet_opportunity()),
                bonus: Some(make_freebet_bonus(BonusStatus::Claimed, 0.0)),
                plan: None,
                rollover: None,
                allocation: None,
                auto_rollover: Some(FreebetAutoRolloverDraft {
                    status: FreebetAutoRolloverStatus::AwaitingFunding,
                    safe_mode: true,
                    execution_allowed: false,
                    required_cash_by_bookmaker: HashMap::from([("pari".into(), 500.0)]),
                    funding_gap_by_bookmaker: HashMap::from([("pari".into(), 125.0)]),
                    funding_readiness: FreebetFundingReadiness {
                        ready: false,
                        total_gap: 125.0,
                        blocking_bookmakers: vec!["pari".into()],
                        largest_gap_bookmaker: Some("pari".into()),
                        largest_gap_amount: Some(125.0),
                    },
                    funding_recommendation: String::new(),
                    trigger: String::new(),
                    next_action: String::new(),
                    read_only_check: String::new(),
                    notes: Vec::new(),
                }),
                rollover_actions: Vec::new(),
                execution_readiness: None,
                updated_at: Utc::now(),
            },
        );

        assert_eq!(
            enriched
                .execution_readiness
                .as_ref()
                .map(|item| &item.stage),
            Some(&FreebetExecutionReadinessStage::FundingBlocked)
        );
        let readiness = enriched.execution_readiness.as_ref().unwrap();
        assert!(!readiness.funding_ready);
        assert!(!readiness.dry_run_ready);
        assert!(readiness
            .blocking_reasons
            .iter()
            .any(|item| item.contains("funding gap")));
    }

    #[test]
    fn generosity_endpoint_returns_all_sports_snapshot() {
        let calc = GenerosityIndexCalc::new();
        let mut football_event = Event {
            id: "football-1".into(),
            sport: Sport::Football,
            league: "Premier League".into(),
            home_team: "A".into(),
            away_team: "B".into(),
            start_time: None,
            is_live: false,
            bookmaker_slug: "pari".into(),
            raw_url: None,
            extra: HashMap::new(),
        };
        let mut tennis_event = football_event.clone();
        football_event.bookmaker_slug = "pari".into();
        tennis_event.id = "tennis-1".into();
        tennis_event.sport = Sport::Tennis;

        let odds = vec![
            shared::Odd {
                id: "football-odd-1".into(),
                event_id: football_event.id.clone(),
                bookmaker_slug: "pari".into(),
                market: "1X2".into(),
                selection: "1".into(),
                odds: 2.0,
                odds_type: shared::odds::OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            shared::Odd {
                id: "football-odd-x".into(),
                event_id: football_event.id.clone(),
                bookmaker_slug: "pari".into(),
                market: "1X2".into(),
                selection: "X".into(),
                odds: 3.4,
                odds_type: shared::odds::OddsType::Draw,
                line: None,
                timestamp: Utc::now(),
            },
            shared::Odd {
                id: "football-odd-2".into(),
                event_id: football_event.id.clone(),
                bookmaker_slug: "pari".into(),
                market: "1X2".into(),
                selection: "2".into(),
                odds: 3.8,
                odds_type: shared::odds::OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
            shared::Odd {
                id: "tennis-odd-a".into(),
                event_id: tennis_event.id.clone(),
                bookmaker_slug: "pari".into(),
                market: "winner".into(),
                selection: "player_a".into(),
                odds: 1.9,
                odds_type: shared::odds::OddsType::Home,
                line: None,
                timestamp: Utc::now(),
            },
            shared::Odd {
                id: "tennis-odd-b".into(),
                event_id: tennis_event.id.clone(),
                bookmaker_slug: "pari".into(),
                market: "winner".into(),
                selection: "player_b".into(),
                odds: 1.9,
                odds_type: shared::odds::OddsType::Away,
                line: None,
                timestamp: Utc::now(),
            },
        ];

        calc.update(&[football_event, tennis_event], &odds);

        let indices = calc.get_all_indices();
        assert_eq!(indices.len(), 2);
        assert!(indices.iter().any(|index| index.sport == Sport::Football));
        assert!(indices.iter().any(|index| index.sport == Sport::Tennis));
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
        assert!(response.preflight.rollout_gate_active);
        assert!(response.preflight.approval_required);
        assert!(response.preflight.submit_blocked_by_safe_mode);
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
        assert!(response.rollout_gate_active);
        assert!(!response.approval_required);
        assert!(!response.submit_blocked_by_safe_mode);
    }
}
