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
    pub sessions_configured: usize,
    pub sessions_authenticated: usize,
    pub balances_cached: usize,
    pub ready_for_execution: usize,
    pub ready_for_dry_run: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOverview {
    pub autobet_status: AutoBetStatus,
    pub accounts: AccountSessionSummary,
    pub recent_placements: ExecutionPlacementSummary,
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
    pub notes: Vec<String>,
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
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BookmakerBalanceRefreshState {
    NoSession,
    SessionNotAuthenticated,
    AuthenticatedBalanceUnavailable,
    CachedBalanceAvailable,
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
    pub opportunity: Option<FreebetOpportunity>,
    pub bonus: Option<BonusInfo>,
    pub plan: Option<FreebetConversionPlan>,
    pub rollover: Option<FreebetRolloverProgress>,
    pub allocation: Option<FreebetBookmakerAllocation>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreebetLifecycleSummary {
    pub total_bookmakers: usize,
    pub opportunities: usize,
    pub active_bonuses: usize,
    pub tracked_plans: usize,
    pub deposit_required_bookmakers: usize,
    pub discovered: usize,
    pub available: usize,
    pub qualified: usize,
    pub planned: usize,
    pub rollover_in_progress: usize,
    pub rollover_completed: usize,
    pub total_freebet_amount: f64,
    pub total_estimated_profit: f64,
    pub generated_at: DateTime<Utc>,
}
