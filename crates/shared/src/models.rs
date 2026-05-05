use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::odds::OddsType;
use crate::sports::Sport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmaker {
    pub slug: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub parser_type: ParserType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BookmakerStatus {
    ScanOnly,
    ExecutionReady,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerMetadata {
    pub slug: String,
    pub name: String,
    pub enabled: bool,
    pub scan_supported: bool,
    pub execution_supported: bool,
    pub status: BookmakerStatus,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParserReadinessStage {
    Production,
    RolloutReady,
    DiagnosticOnly,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Pass,
    Warn,
    Fail,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserDiagnosticCheck {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserReadiness {
    pub stage: ParserReadinessStage,
    pub production_enabled: bool,
    pub self_check_available: bool,
    pub checks: Vec<ParserDiagnosticCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserCoverage {
    pub slug: String,
    pub name: String,
    pub enabled: bool,
    pub scan_supported: bool,
    pub execution_supported: bool,
    pub status: BookmakerStatus,
    pub parser_type: String,
    pub source: String,
    pub notes: Option<String>,
    pub readiness: Option<ParserReadiness>,
    pub runtime_health: Option<ParserHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserCapabilityCatalogEntry {
    pub slug: String,
    pub name: String,
    pub parser_type: String,
    pub source: String,
    pub status: BookmakerStatus,
    pub scan_supported: bool,
    pub execution_supported: bool,
    pub readiness_stage: Option<ParserReadinessStage>,
    pub production_enabled: bool,
    pub self_check_available: bool,
    pub health_status: Option<HealthStatus>,
    pub top_issue: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BookmakerTriageBucket {
    Ready,
    Unfinished,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerStatusCatalogEntry {
    pub slug: String,
    pub name: String,
    pub triage_bucket: BookmakerTriageBucket,
    pub status: BookmakerStatus,
    pub parser_type: String,
    pub source: String,
    pub enabled: bool,
    pub scan_supported: bool,
    pub execution_supported: bool,
    pub readiness_stage: Option<ParserReadinessStage>,
    pub production_enabled: bool,
    pub nightly_kpi_gate: ParserNightlyKpiGate,
    pub health_status: Option<HealthStatus>,
    pub top_issue: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookmakerStatusCatalogSummary {
    pub total: usize,
    pub ready: usize,
    pub unfinished: usize,
    pub disabled: usize,
    pub production_enabled: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerStatusCatalog {
    pub summary: BookmakerStatusCatalogSummary,
    pub bookmakers: Vec<BookmakerStatusCatalogEntry>,
}

impl BookmakerMetadata {
    pub fn new(
        slug: impl Into<String>,
        name: impl Into<String>,
        enabled: bool,
        scan_supported: bool,
        execution_supported: bool,
        notes: Option<String>,
    ) -> Self {
        let status = if !enabled || !scan_supported {
            BookmakerStatus::Disabled
        } else if execution_supported {
            BookmakerStatus::ExecutionReady
        } else {
            BookmakerStatus::ScanOnly
        };

        Self {
            slug: slug.into(),
            name: name.into(),
            enabled,
            scan_supported,
            execution_supported,
            status,
            notes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParserType {
    Http,
    Playwright,
    WebSocket,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub sport: Sport,
    pub league: String,
    pub home_team: String,
    pub away_team: String,
    pub start_time: Option<DateTime<Utc>>,
    pub is_live: bool,
    pub bookmaker_slug: String,
    pub raw_url: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Odd {
    pub id: String,
    pub event_id: String,
    pub bookmaker_slug: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub odds_type: OddsType,
    pub line: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventWithOdds {
    pub event: Event,
    pub odds: Vec<Odd>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Surebet {
    pub id: Uuid,
    pub sport: Sport,
    pub league: String,
    pub home_team: String,
    pub away_team: String,
    pub start_time: Option<DateTime<Utc>>,
    pub is_live: bool,
    pub profit_percent: f64,
    pub total_stake: f64,
    pub legs: Vec<SurebetLeg>,
    pub detected_at: DateTime<Utc>,
    pub verified: bool,
    pub mirror: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurebetLeg {
    pub bookmaker: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub line: Option<f64>,
    pub stake: f64,
    pub payout: f64,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetOpportunity {
    pub id: Uuid,
    pub bookmaker: String,
    pub hedge_bookmaker: String,
    pub event: Event,
    pub market: String,
    pub selection: String,
    pub hedge_selection: String,
    pub back_odds: f64,
    pub lay_odds: f64,
    pub freebet_amount: f64,
    pub guaranteed_profit: f64,
    pub roi: f64,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerosityIndex {
    pub bookmaker: String,
    pub sport: Sport,
    pub avg_margin: f64,
    pub avg_odds: f64,
    pub best_odds_count: usize,
    pub total_events: usize,
    pub score: f64,
    pub updated_at: DateTime<Utc>,
}

impl GenerosityIndex {
    pub fn get_all_indices(&self) -> Vec<GenerosityIndex> {
        vec![self.clone()]
    }
}

impl GenerosityIndex {
    pub fn new() -> Self {
        GenerosityIndex {
            bookmaker: String::new(),
            sport: Sport::Football,
            avg_margin: 0.0,
            avg_odds: 0.0,
            best_odds_count: 0,
            total_events: 0,
            score: 0.0,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirrorLine {
    pub id: Uuid,
    pub market: String,
    pub line: f64,
    pub bookmaker_a: String,
    pub odds_a: f64,
    pub bookmaker_b: String,
    pub odds_b: f64,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OddsError {
    pub id: Uuid,
    pub bookmaker: String,
    pub event: Event,
    pub market: String,
    pub selection: String,
    pub suspicious_odds: f64,
    pub avg_market_odds: f64,
    pub deviation_percent: f64,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorridorOpportunity {
    pub id: Uuid,
    pub sport: Sport,
    pub league: String,
    pub home_team: String,
    pub away_team: String,
    pub start_time: Option<DateTime<Utc>>,
    pub is_live: bool,
    pub bookmaker_a: String,
    pub bookmaker_b: String,
    pub market: String,
    pub line_a: f64,
    pub odds_a: f64,
    pub line_b: f64,
    pub odds_b: f64,
    pub corridor_size: f64,
    pub double_win_probability: f64,
    pub expected_roi: f64,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueBet {
    pub id: Uuid,
    pub bookmaker: String,
    pub event: Event,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub fair_odds: f64,
    pub edge_percent: f64,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpportunityKind {
    Surebet2way,
    Surebet3way,
    CrossMarket,
    Corridor,
    ExpressFork,
    ValueBet,
    Middle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedOpportunity {
    pub id: Uuid,
    pub kind: OpportunityKind,
    pub sport: Option<Sport>,
    pub league: Option<String>,
    pub home_team: Option<String>,
    pub away_team: Option<String>,
    pub bookmaker_pairs: Vec<String>,
    pub market: Option<String>,
    pub expected_value: f64,
    pub profit_percent: Option<f64>,
    pub detected_at: DateTime<Utc>,
    pub correlation_blocked: bool,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserHealth {
    pub bookmaker: String,
    pub status: HealthStatus,
    pub last_success: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub avg_response_time_ms: f64,
    pub events_parsed: u64,
    pub uptime_percent: f64,
    pub readiness: Option<ParserReadiness>,
    pub diagnostics: Vec<ParserDiagnosticCheck>,
}

/// Compact parser promotion KPI strip for operator surfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserPromotionKpi {
    pub production: u32,
    pub rollout_ready: u32,
    pub diagnostic_only: u32,
    pub blocked: u32,
    pub total: u32,
    pub nightly_kpi: ParserNightlyKpiSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParserNightlyKpiGateStatus {
    Pass,
    Warn,
    Fail,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserNightlyKpiGate {
    pub status: ParserNightlyKpiGateStatus,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParserNightlyKpiSummary {
    pub pass: u32,
    pub warn: u32,
    pub fail: u32,
    pub unavailable: u32,
    pub total: u32,
}

impl ParserPromotionKpi {
    pub fn from_coverage(coverage: &[ParserCoverage]) -> Self {
        let mut production = 0u32;
        let mut rollout_ready = 0u32;
        let mut diagnostic_only = 0u32;
        let mut blocked = 0u32;
        let mut nightly_kpi = ParserNightlyKpiSummary::default();

        for c in coverage {
            if let Some(ref readiness) = c.readiness {
                match readiness.stage {
                    ParserReadinessStage::Production => production += 1,
                    ParserReadinessStage::RolloutReady => rollout_ready += 1,
                    ParserReadinessStage::DiagnosticOnly => diagnostic_only += 1,
                    ParserReadinessStage::Blocked => blocked += 1,
                }
            } else if c.enabled {
                // Default enabled parsers to diagnostic_only if no readiness set
                diagnostic_only += 1;
            }

            match ParserNightlyKpiGate::from_readiness(c.readiness.as_ref()).status {
                ParserNightlyKpiGateStatus::Pass => nightly_kpi.pass += 1,
                ParserNightlyKpiGateStatus::Warn => nightly_kpi.warn += 1,
                ParserNightlyKpiGateStatus::Fail => nightly_kpi.fail += 1,
                ParserNightlyKpiGateStatus::Unavailable => nightly_kpi.unavailable += 1,
            }
        }

        let total = production + rollout_ready + diagnostic_only + blocked;
        nightly_kpi.total = coverage.len() as u32;
        Self {
            production,
            rollout_ready,
            diagnostic_only,
            blocked,
            total,
            nightly_kpi,
        }
    }
}

impl ParserNightlyKpiGate {
    pub fn from_readiness(readiness: Option<&ParserReadiness>) -> Self {
        let Some(readiness) = readiness else {
            return Self {
                status: ParserNightlyKpiGateStatus::Unavailable,
                note: None,
            };
        };

        let nightly_checks = readiness
            .checks
            .iter()
            .filter(|check| is_nightly_kpi_check(check))
            .collect::<Vec<_>>();

        let pick_note = |severity: DiagnosticSeverity| {
            nightly_checks
                .iter()
                .find(|check| check.severity == severity)
                .map(|check| check.message.clone())
        };

        if nightly_checks.is_empty() {
            return Self {
                status: ParserNightlyKpiGateStatus::Unavailable,
                note: None,
            };
        }

        if let Some(note) = pick_note(DiagnosticSeverity::Fail) {
            return Self {
                status: ParserNightlyKpiGateStatus::Fail,
                note: Some(note),
            };
        }

        if let Some(note) = pick_note(DiagnosticSeverity::Warn) {
            return Self {
                status: ParserNightlyKpiGateStatus::Warn,
                note: Some(note),
            };
        }

        if let Some(note) = pick_note(DiagnosticSeverity::Pass) {
            return Self {
                status: ParserNightlyKpiGateStatus::Pass,
                note: Some(note),
            };
        }

        Self {
            status: ParserNightlyKpiGateStatus::Unavailable,
            note: nightly_checks.first().map(|check| check.message.clone()),
        }
    }
}

fn is_nightly_kpi_check(check: &ParserDiagnosticCheck) -> bool {
    check.code.contains("nightly")
        || check.code.contains("runtime_kpi")
        || check.code.contains("live_kpi")
        || check.code.contains("prematch_kpi")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCircuitState {
    Closed,
    HalfOpen,
    Open,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserRuntimeSnapshot {
    pub bookmaker: String,
    pub last_attempt: Option<DateTime<Utc>>,
    pub last_success: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_result_status: ParserResultStatus,
    pub last_result_message: Option<String>,
    pub validation_checks: Vec<ParserDiagnosticCheck>,
    pub consecutive_failures: u32,
    pub avg_response_time_ms: f64,
    pub events_parsed: u64,
    pub odds_parsed: u64,
    pub uptime_percent: f64,
    pub total_runs: u64,
    pub successful_runs: u64,
    pub circuit_state: RuntimeCircuitState,
}

impl ParserRuntimeSnapshot {
    pub fn latest_activity(&self) -> Option<DateTime<Utc>> {
        match (self.last_attempt, self.last_success) {
            (Some(last_attempt), Some(last_success)) => Some(last_attempt.max(last_success)),
            (Some(last_attempt), None) => Some(last_attempt),
            (None, Some(last_success)) => Some(last_success),
            (None, None) => None,
        }
    }

    pub fn staleness_age_secs(&self, now: DateTime<Utc>) -> Option<i64> {
        self.latest_activity()
            .map(|activity| now.signed_duration_since(activity).num_seconds().max(0))
    }

    pub fn is_stale(&self, now: DateTime<Utc>, stale_after_secs: u64) -> bool {
        self.total_runs > 0
            && stale_after_secs > 0
            && self
                .staleness_age_secs(now)
                .is_some_and(|age_secs| age_secs >= stale_after_secs as i64)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParserResultStatus {
    Healthy,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    CircuitOpen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannerMetrics {
    pub cycle_time_ms: u64,
    pub events_parsed: usize,
    pub surebets_found: usize,
    pub active_bookmakers: usize,
    pub failed_bookmakers: usize,
    pub cache_hit_rate: f64,
    pub memory_mb: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressFork {
    pub id: Uuid,
    pub profit_percent: f64,
    pub total_stake: f64,
    pub legs: Vec<ExpressForkLeg>,
    pub detected_at: DateTime<Utc>,
    pub verified: bool,
    pub risk_level: ExpressForkRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressForkLeg {
    pub bookmaker: String,
    pub event: Event,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub stake: f64,
    pub is_express: bool,
    pub express_events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpressForkRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusInfo {
    pub id: Uuid,
    pub bookmaker: String,
    pub bonus_type: BonusType,
    pub name: String,
    pub amount: f64,
    pub currency: String,
    pub wager_requirement: f64,
    pub min_odds: f64,
    pub max_bet: f64,
    pub expiry_days: u32,
    pub real_value: f64,
    pub ev: f64,
    pub difficulty: BonusDifficulty,
    pub status: BonusStatus,
    pub wager_progress: f64,
    pub detected_at: DateTime<Utc>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BonusType {
    Welcome,
    Reload,
    Freebet,
    Cashback,
    Insurance,
    OddsBoost,
    Loyalty,
    Special,
    RefundOnDrawZeroZero,
    EarlyPayoutUpByTwo,
    BoostedOdds,
    AccaInsurance,
    DepositBonus,
    FreebetQualification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreRange {
    pub from: i32,
    pub to: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusRequirements {
    pub min_odds: Option<f64>,
    pub wagering_multiplier: Option<f64>,
    pub turnover_target: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddleOpportunityV2 {
    pub id: Uuid,
    pub event_id: String,
    pub bookmaker_a: String,
    pub bookmaker_b: String,
    pub market_a: String,
    pub market_b: String,
    pub odds_a: f64,
    pub odds_b: f64,
    pub win_win_scenario: Option<ScoreRange>,
    pub loss_win_scenario: Option<ScoreRange>,
    pub expected_value: f64,
    pub max_profit: f64,
    pub max_loss: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusOpportunity {
    pub id: Uuid,
    pub bonus_type: BonusType,
    pub bookmaker: String,
    pub event_id: Option<String>,
    pub description: String,
    pub nominal_value: f64,
    pub probability_trigger: f64,
    pub expected_value: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub requirements: BonusRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FreebetStatus {
    Discovered,
    Qualifying,
    Qualified,
    Wagering,
    Completed,
    Expired,
    Lost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetLifecycleV2 {
    pub id: Uuid,
    pub bookmaker: String,
    pub amount: f64,
    pub status: FreebetStatus,
    pub qualification_progress: Option<f64>,
    pub wagering_progress: Option<f64>,
    pub expiry_date: DateTime<Utc>,
    pub expected_conversion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BonusDifficulty {
    Easy,
    Medium,
    Hard,
    VeryHard,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BonusStatus {
    Available,
    Claimed,
    Wagering,
    Completed,
    Expired,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankrollState {
    pub total_budget: f64,
    pub bookmakers: Vec<BookmakerBalance>,
    pub total_exposure: f64,
    pub daily_profit: f64,
    pub daily_loss: f64,
    pub total_profit: f64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerBalance {
    pub bookmaker: String,
    pub balance: f64,
    pub exposure: f64,
    pub available: f64,
    pub recommended_deposit: f64,
    pub recommended_withdraw: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusPlan {
    pub id: Uuid,
    pub bookmaker: String,
    pub bonus_name: String,
    pub bonus_amount: f64,
    pub wager_required: f64,
    pub wager_done: f64,
    pub progress_percent: f64,
    pub estimated_profit: f64,
    pub steps: Vec<BonusStep>,
    pub created_at: DateTime<Utc>,
    pub status: BonusStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusStep {
    pub step_number: u32,
    pub description: String,
    pub market: String,
    pub selection: String,
    pub bookmaker: String,
    pub odds: f64,
    pub stake: f64,
    pub status: BonusStepStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BonusStepStatus {
    Pending,
    Placed,
    Won,
    Lost,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBetStatus {
    pub enabled: bool,
    pub running: bool,
    pub bets_placed_today: u32,
    pub bets_placed_total: u64,
    pub profit_today: f64,
    pub profit_total: f64,
    pub last_bet: Option<DateTime<Utc>>,
    pub errors_today: u32,
    pub emergency_stopped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlacementSummary {
    pub total: usize,
    pub pending: usize,
    pub placed: usize,
    pub settled: usize,
    pub cancelled: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSessionSummary {
    pub total_bookmakers: usize,
    pub accounts_configured: usize,
    pub accounts_enabled: usize,
    pub disabled_accounts: usize,
    pub accounts_with_control_issues: usize,
    pub sessions_configured: usize,
    pub sessions_authenticated: usize,
    pub sessions_stale: usize,
    pub balances_cached: usize,
    pub balances_stale: usize,
    pub auth_snapshots_stale: usize,
    pub ready_for_execution: usize,
    pub ready_for_dry_run: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOverview {
    pub autobet_status: AutoBetStatus,
    pub accounts: AccountSessionSummary,
    pub recent_placements: ExecutionPlacementSummary,
    pub ledger_placements: ExecutionPlacementSummary,
    pub state_machine: ExecutionStateMachineMetadata,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionStatePhaseSummary {
    pub pending_placement: usize,
    pub confirmed_placement: usize,
    pub settled: usize,
    pub cancelled: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStateSnapshotRecord {
    pub placement_id: Uuid,
    pub bookmaker: String,
    pub phase: String,
    pub placement_status: BetStatus,
    pub sequence: u64,
    pub updated_at: DateTime<Utc>,
    pub last_action: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionStateMachineMetadata {
    pub total_snapshots: usize,
    pub total_transitions: usize,
    pub latest_snapshot_at: Option<DateTime<Utc>>,
    pub latest_transition_at: Option<DateTime<Utc>>,
    pub phases: ExecutionStatePhaseSummary,
    pub recent_snapshots: Vec<ExecutionStateSnapshotRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStateTransitionRecord {
    pub placement_id: Uuid,
    pub bookmaker: String,
    pub from_phase: Option<String>,
    pub to_phase: String,
    pub placement_status: BetStatus,
    pub sequence: u64,
    pub action: String,
    pub occurred_at: DateTime<Utc>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionBookmakerStateSummary {
    pub bookmaker: String,
    pub total_snapshots: usize,
    pub phases: ExecutionStatePhaseSummary,
    pub latest_snapshot_at: Option<DateTime<Utc>>,
    pub latest_transition_at: Option<DateTime<Utc>>,
    pub latest_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionStateReadinessSummary {
    pub total_bookmakers: usize,
    pub accounts_configured: usize,
    pub accounts_enabled: usize,
    pub auth_ready: usize,
    pub sessions_authenticated: usize,
    pub sessions_stale: usize,
    pub balances_cached: usize,
    pub balances_stale: usize,
    pub auth_snapshots_stale: usize,
    pub dry_run_ready: usize,
    pub placement_ready: usize,
    pub approval_required: usize,
    pub submit_blocked_by_safe_mode: usize,
    pub operator_attention_required: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionBookmakerReadinessRecord {
    pub bookmaker: String,
    pub account_configured: bool,
    pub account_enabled: bool,
    pub execution_mode: Option<BookmakerExecutionMode>,
    pub requires_session: bool,
    pub auth_ready: bool,
    pub session_authenticated: bool,
    pub session_stale: bool,
    pub balance_cached: bool,
    pub balance_stale: bool,
    pub auth_snapshot_stale: bool,
    pub dry_run_ready: bool,
    pub placement_ready: bool,
    pub approval_required: bool,
    pub submit_blocked_by_safe_mode: bool,
    pub persistence_warnings: Vec<String>,
    pub operator_action: Option<String>,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStateAudit {
    pub total_snapshots: usize,
    pub total_transitions: usize,
    pub latest_snapshot_at: Option<DateTime<Utc>>,
    pub latest_transition_at: Option<DateTime<Utc>>,
    pub readiness: ExecutionStateReadinessSummary,
    pub bookmaker_readiness: Vec<ExecutionBookmakerReadinessRecord>,
    pub bookmaker_summaries: Vec<ExecutionBookmakerStateSummary>,
    pub recent_transitions: Vec<ExecutionStateTransitionRecord>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOperatorQueueItem {
    pub bookmaker: String,
    pub severity: String,
    pub priority_score: i32,
    pub execution_mode: Option<BookmakerExecutionMode>,
    pub placement_ready: bool,
    pub approval_required: bool,
    pub submit_blocked_by_safe_mode: bool,
    pub session_stale: bool,
    pub balance_stale: bool,
    pub auth_snapshot_stale: bool,
    pub operator_action: Option<String>,
    pub blocking_reasons: Vec<String>,
    pub persistence_warnings: Vec<String>,
    pub latest_error: Option<String>,
    pub latest_transition_at: Option<DateTime<Utc>>,
    pub latest_snapshot_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOperatorQueueAudit {
    pub total_items: usize,
    pub critical_items: usize,
    pub warning_items: usize,
    pub items: Vec<ExecutionOperatorQueueItem>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLedgerRecord {
    pub placement: BetPlacement,
    pub action: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLedgerAudit {
    pub total_entries: usize,
    pub unique_placements: usize,
    pub latest_recorded_at: Option<DateTime<Utc>>,
    pub state_machine: ExecutionStateMachineMetadata,
    pub recent_records: Vec<ExecutionLedgerRecord>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookmakerExecutionMode {
    NoOp,
    Disabled,
    DryRun,
    Armed,
    SemiRealReady,
    Real,
}

impl BookmakerExecutionMode {
    pub fn allows_dry_run(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub fn is_armed(&self) -> bool {
        matches!(self, Self::Armed | Self::SemiRealReady | Self::Real)
    }

    pub fn allows_submission_path(&self) -> bool {
        matches!(self, Self::SemiRealReady | Self::Real)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookmakerSessionState {
    Configured,
    Active,
    Expired,
    Locked,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookmakerSessionSyncState {
    NoSession,
    Configured,
    Authenticated,
    Expired,
    Locked,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerAccountCapabilityMetadata {
    pub api_base_url: Option<String>,
    pub planned_endpoints: Vec<String>,
    pub supports_read_only_session_sync: bool,
    pub supports_read_only_balance_refresh: bool,
    pub remote_balance_fetch_enabled: bool,
    pub auth: BookmakerAdapterAuthMetadata,
    pub readiness: BookmakerAdapterReadinessMetadata,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BookmakerAuthState {
    NoSession,
    Configured,
    Authenticated,
    Expired,
    Locked,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerAdapterAuthMetadata {
    pub flow: String,
    pub requires_human_bootstrap: bool,
    pub session_bootstrap_enabled: bool,
    pub session_refresh_enabled: bool,
    pub persisted_snapshot_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BookmakerAdapterReadinessStage {
    Stub,
    SessionBootstrapPending,
    AuthenticatedReadOnly,
    SafeModePlacementReady,
    RealMoneyReady,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerAdapterReadinessMetadata {
    pub stage: BookmakerAdapterReadinessStage,
    pub safe_mode_only: bool,
    pub approval_reference_required: bool,
    pub operator_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerExecutionCapability {
    pub bookmaker: String,
    pub supports_dry_run: bool,
    pub supports_balance_snapshot: bool,
    pub supports_bet_placement: bool,
    pub supports_real_money: bool,
    pub requires_session: bool,
    pub account_metadata: BookmakerAccountCapabilityMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerSessionStatus {
    pub account_id: Option<Uuid>,
    pub bookmaker: String,
    pub sync_state: BookmakerSessionSyncState,
    pub authenticated: bool,
    pub can_refresh_balance: bool,
    pub detail: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerAccount {
    pub id: Uuid,
    pub bookmaker: String,
    pub label: String,
    pub currency: String,
    pub enabled: bool,
    pub mode: BookmakerExecutionMode,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerSession {
    pub account_id: Uuid,
    pub bookmaker: String,
    pub state: BookmakerSessionState,
    pub token_hint: Option<String>,
    pub last_synced_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerBalanceSnapshot {
    pub account_id: Uuid,
    pub bookmaker: String,
    pub currency: String,
    pub total_balance: f64,
    pub available_balance: f64,
    pub exposure: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bonus_balance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source: Option<String>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookmakerBalanceRefreshState {
    NoSession,
    SessionNotAuthenticated,
    AuthenticatedBalanceUnavailable,
    CachedBalanceAvailable,
    RemoteBalanceFetched,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerBalanceRefresh {
    pub account_id: Option<Uuid>,
    pub bookmaker: String,
    pub state: BookmakerBalanceRefreshState,
    pub session_status: BookmakerSessionStatus,
    pub snapshot: Option<BookmakerBalanceSnapshot>,
    pub detail: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmakerAuthSnapshot {
    pub account_id: Option<Uuid>,
    pub bookmaker: String,
    pub auth_state: BookmakerAuthState,
    pub readiness_stage: BookmakerAdapterReadinessStage,
    pub mode: Option<BookmakerExecutionMode>,
    pub enabled: bool,
    pub cached_balance_available: bool,
    pub submit_enabled: bool,
    pub real_money_enabled: bool,
    pub safe_mode_blocked: bool,
    pub session_last_synced_at: Option<DateTime<Utc>>,
    pub balance_captured_at: Option<DateTime<Utc>>,
    pub last_authenticated_at: Option<DateTime<Utc>>,
    pub detail: Option<String>,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetExecutionRequest {
    pub bookmaker: String,
    pub event_id: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub stake: f64,
    pub allow_dry_run: bool,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BetExecutionStatus {
    Pending,
    DryRun,
    Armed,
    Blocked,
    Submitted,
    Accepted,
    Rejected,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetExecutionReceipt {
    pub ticket_id: Option<String>,
    pub account_id: Option<Uuid>,
    pub bookmaker: String,
    pub status: BetExecutionStatus,
    pub mode: BookmakerExecutionMode,
    pub accepted_stake: f64,
    pub accepted_odds: f64,
    pub message: Option<String>,
    pub placed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetPlacement {
    pub id: Uuid,
    pub bookmaker: String,
    pub event: Event,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub stake: f64,
    pub status: BetStatus,
    pub placed_at: DateTime<Utc>,
    #[serde(default)]
    pub execution: Option<BetExecutionReceipt>,
    pub result: Option<BetResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BetStatus {
    Pending,
    Placed,
    Settled,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BetResult {
    Won(f64),
    Lost,
    Void,
    Cashout(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StakeValidationDecision {
    Accept,
    Adjust,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeValidationRequest {
    pub bookmaker: String,
    pub desired_stake: f64,
    pub min_stake: Option<f64>,
    pub max_stake: Option<f64>,
    pub bookmaker_available_balance: Option<f64>,
    pub bankroll_available_balance: Option<f64>,
    pub allow_auto_adjust: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeValidationResult {
    pub decision: StakeValidationDecision,
    pub adjusted_stake: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeValidationPreflightRequest {
    pub bookmaker: String,
    pub desired_stake: f64,
    pub min_stake: Option<f64>,
    pub max_stake: Option<f64>,
    pub bankroll_available_balance: Option<f64>,
    pub allow_auto_adjust: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeValidationPreflightResponse {
    pub bookmaker: String,
    pub capability: BookmakerExecutionCapability,
    pub account: Option<BookmakerAccount>,
    pub balance_refresh: BookmakerBalanceRefresh,
    pub validation: StakeValidationResult,
    pub executable: bool,
    pub dry_run_ready: bool,
    pub arm_required: bool,
    pub armed_for_execution: bool,
    pub placement_ready: bool,
    pub real_money_enabled: bool,
    pub rollout_gate_active: bool,
    pub approval_required: bool,
    pub submit_blocked_by_safe_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBetDryRunLegRequest {
    pub bookmaker: String,
    pub event_id: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub desired_stake: f64,
    pub min_stake: Option<f64>,
    pub max_stake: Option<f64>,
    pub bankroll_available_balance: Option<f64>,
    pub allow_auto_adjust: bool,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBetDryRunRequest {
    pub legs: Vec<AutoBetDryRunLegRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBetDryRunLegResponse {
    pub preflight: StakeValidationPreflightResponse,
    pub execution_request: BetExecutionRequest,
    pub receipt: Option<BetExecutionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoBetDryRunResponse {
    pub legs: Vec<AutoBetDryRunLegResponse>,
    pub all_legs_executable: bool,
    pub ready_legs: usize,
    pub rejected_legs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemiAutoCouponStatus {
    AwaitingOperator,
    Blocked,
    AppliedSafeMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemiAutoCouponLeg {
    pub bookmaker: String,
    pub event_id: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub stake: f64,
    pub url: Option<String>,
    pub preflight: StakeValidationPreflightResponse,
    pub execution_request: BetExecutionRequest,
    pub receipt: Option<BetExecutionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemiAutoCoupon {
    pub id: Uuid,
    pub surebet_id: Uuid,
    pub status: SemiAutoCouponStatus,
    pub profit_percent: f64,
    pub total_stake: f64,
    pub league: String,
    pub home_team: String,
    pub away_team: String,
    pub is_live: bool,
    pub operator_required: bool,
    pub safe_mode: bool,
    pub all_legs_ready: bool,
    pub blocking_reasons: Vec<String>,
    pub legs: Vec<SemiAutoCouponLeg>,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DepositUrgency {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositAllocationTarget {
    pub bookmaker: String,
    pub current_available: f64,
    pub target_available: f64,
    pub recommended_deposit: f64,
    pub deposit_gap: f64,
    pub urgency: DepositUrgency,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositAllocationGuidance {
    pub total_budget_limit: f64,
    pub current_available_total: f64,
    pub target_per_bookmaker: f64,
    pub total_recommended_deposit: f64,
    pub targets: Vec<DepositAllocationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetPlanRequest {
    pub freebet_bookmaker: String,
    pub qualifying_bookmaker: String,
    pub hedge_bookmaker: String,
    pub market: String,
    pub qualifying_selection: String,
    pub freebet_selection: String,
    pub hedge_selection: String,
    pub freebet_amount: f64,
    pub qualifying_odds: f64,
    pub back_odds: f64,
    pub lay_odds: f64,
    pub estimated_qualifying_loss: f64,
    pub exchange_like_hedge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FreebetStepType {
    QualifyingBet,
    FreebetBet,
    Hedge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetPlanStep {
    pub step_number: u32,
    pub step_type: FreebetStepType,
    pub bookmaker: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub stake: f64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetHedgeLeg {
    pub bookmaker: String,
    pub market: String,
    pub selection: String,
    pub odds: f64,
    pub stake: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetConversionPlan {
    pub id: Uuid,
    pub bookmaker: String,
    pub freebet_amount: f64,
    pub qualifying_cost: f64,
    pub conversion_rate: f64,
    pub estimated_profit: f64,
    #[serde(default)]
    pub required_cash_by_bookmaker: HashMap<String, f64>,
    #[serde(default)]
    pub funding_recommendation: String,
    pub hedge: FreebetHedgeLeg,
    pub steps: Vec<FreebetPlanStep>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FreebetLifecycleStage {
    Discovered,
    Available,
    Qualified,
    Planned,
    RolloverInProgress,
    RolloverCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FreebetProgressStatus {
    NotStarted,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FreebetAutoRolloverStatus {
    DraftOnly,
    AwaitingFunding,
    AwaitingTrigger,
    Monitoring,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FreebetLifecycleActionStatus {
    Pending,
    Ready,
    Monitoring,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetLifecycleAction {
    pub key: String,
    pub label: String,
    pub status: FreebetLifecycleActionStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FreebetExecutionReadinessStage {
    Untracked,
    FundingBlocked,
    AwaitingManualTrigger,
    ReadOnlyReady,
    MonitoringOnly,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetExecutionReadiness {
    pub stage: FreebetExecutionReadinessStage,
    pub account_configured: bool,
    pub session_required: bool,
    pub session_ready: bool,
    pub balance_snapshot_available: bool,
    pub dry_run_ready: bool,
    pub funding_ready: bool,
    pub manual_trigger_required: bool,
    pub monitoring_only: bool,
    pub real_money_enabled: bool,
    pub submit_blocked_by_safe_mode: bool,
    #[serde(default)]
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FreebetFundingReadiness {
    pub ready: bool,
    pub total_gap: f64,
    #[serde(default)]
    pub blocking_bookmakers: Vec<String>,
    pub largest_gap_bookmaker: Option<String>,
    pub largest_gap_amount: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetAutoRolloverDraft {
    pub status: FreebetAutoRolloverStatus,
    pub safe_mode: bool,
    pub execution_allowed: bool,
    #[serde(default)]
    pub required_cash_by_bookmaker: HashMap<String, f64>,
    #[serde(default)]
    pub funding_gap_by_bookmaker: HashMap<String, f64>,
    #[serde(default)]
    pub funding_readiness: FreebetFundingReadiness,
    #[serde(default)]
    pub funding_recommendation: String,
    pub trigger: String,
    #[serde(default)]
    pub next_action: String,
    #[serde(default)]
    pub read_only_check: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetRolloverProgress {
    pub required_turnover: f64,
    pub completed_turnover: f64,
    pub remaining_turnover: f64,
    pub progress_percent: f64,
    pub status: FreebetProgressStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetBookmakerAllocation {
    pub bookmaker: String,
    pub available_balance: Option<f64>,
    pub recommended_deposit: Option<f64>,
    pub deposit_gap: Option<f64>,
    pub urgency: Option<DepositUrgency>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetLifecycleState {
    pub bookmaker: String,
    pub lifecycle_stage: FreebetLifecycleStage,
    #[serde(default)]
    pub next_milestone: String,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub read_only_follow_up: String,
    #[serde(default)]
    pub read_only_focus: String,
    pub opportunity: Option<FreebetOpportunity>,
    pub bonus: Option<BonusInfo>,
    pub plan: Option<FreebetConversionPlan>,
    pub rollover: Option<FreebetRolloverProgress>,
    pub allocation: Option<FreebetBookmakerAllocation>,
    #[serde(default)]
    pub auto_rollover: Option<FreebetAutoRolloverDraft>,
    #[serde(default)]
    pub rollover_actions: Vec<FreebetLifecycleAction>,
    #[serde(default)]
    pub execution_readiness: Option<FreebetExecutionReadiness>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetLifecycleLabelCount {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetLifecycleFundingGapLeader {
    pub bookmaker: String,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetLifecycleSummary {
    pub total_bookmakers: usize,
    pub opportunities: usize,
    pub active_bonuses: usize,
    pub tracked_plans: usize,
    pub deposit_required_bookmakers: usize,
    pub blocked_states: usize,
    #[serde(default)]
    pub total_funding_gap: f64,
    #[serde(default)]
    pub largest_funding_gap: Option<FreebetLifecycleFundingGapLeader>,
    pub discovered: usize,
    pub available: usize,
    pub qualified: usize,
    pub planned: usize,
    pub rollover_in_progress: usize,
    pub rollover_completed: usize,
    #[serde(default)]
    pub next_milestones: Vec<FreebetLifecycleLabelCount>,
    #[serde(default)]
    pub blockers: Vec<FreebetLifecycleLabelCount>,
    #[serde(default)]
    pub read_only_focuses: Vec<FreebetLifecycleLabelCount>,
    pub total_freebet_amount: f64,
    pub total_estimated_profit: f64,
    pub generated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage_with_readiness(
        slug: &str,
        stage: ParserReadinessStage,
        enabled: bool,
        checks: Vec<ParserDiagnosticCheck>,
    ) -> ParserCoverage {
        ParserCoverage {
            slug: slug.to_string(),
            name: slug.to_string(),
            enabled,
            scan_supported: enabled,
            execution_supported: false,
            status: if enabled {
                BookmakerStatus::ScanOnly
            } else {
                BookmakerStatus::Disabled
            },
            parser_type: "api".to_string(),
            source: format!("crates/parsers/src/{slug}.rs"),
            notes: None,
            readiness: Some(ParserReadiness {
                stage: stage.clone(),
                production_enabled: matches!(stage, ParserReadinessStage::Production),
                self_check_available: true,
                checks,
            }),
            runtime_health: None,
        }
    }

    #[test]
    fn parser_promotion_kpi_counts_nightly_gate_statuses() {
        let kpi = ParserPromotionKpi::from_coverage(&[
            coverage_with_readiness(
                "baltbet",
                ParserReadinessStage::Production,
                true,
                vec![ParserDiagnosticCheck {
                    code: "strict_live_kpi_recently_met".to_string(),
                    severity: DiagnosticSeverity::Pass,
                    message: "nightly passed".to_string(),
                }],
            ),
            coverage_with_readiness(
                "zenit",
                ParserReadinessStage::RolloutReady,
                true,
                vec![ParserDiagnosticCheck {
                    code: "strict_nightly_regressed_to_zero".to_string(),
                    severity: DiagnosticSeverity::Warn,
                    message: "nightly regressed".to_string(),
                }],
            ),
            coverage_with_readiness(
                "melbet",
                ParserReadinessStage::Blocked,
                false,
                vec![ParserDiagnosticCheck {
                    code: "bootstrap_unverified".to_string(),
                    severity: DiagnosticSeverity::Fail,
                    message: "not a nightly check".to_string(),
                }],
            ),
        ]);

        assert_eq!(kpi.production, 1);
        assert_eq!(kpi.rollout_ready, 1);
        assert_eq!(kpi.blocked, 1);
        assert_eq!(kpi.nightly_kpi.pass, 1);
        assert_eq!(kpi.nightly_kpi.warn, 1);
        assert_eq!(kpi.nightly_kpi.unavailable, 1);
        assert_eq!(kpi.nightly_kpi.total, 3);
    }
}
