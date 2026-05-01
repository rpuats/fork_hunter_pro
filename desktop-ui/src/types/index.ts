export interface ApiResponse<T> {
  success: boolean
  data: T | null
  error: string | null
  timestamp: string
}

export type UiBookmakerStatus = 'active' | 'inactive' | 'error'
export type BackendBookmakerStatus = 'scan_only' | 'execution_ready' | 'disabled'
export type ExpressRiskLevel = 'low' | 'medium' | 'high'
export type BackendExpressRiskLevel = ExpressRiskLevel | 'Low' | 'Medium' | 'High'

export interface Surebet {
  id: string
  sport: string
  league: string
  home_team: string
  away_team: string
  start_time: string | null
  is_live: boolean
  profit_percent: number
  profitPercent: number
  total_stake: number
  legs: SurebetLeg[]
  detected_at: string
  verified: boolean
  mirror: boolean
}

export interface BackendSurebet extends Omit<Surebet, 'id' | 'sport' | 'profitPercent'> {
  id: string
  sport: string
}

export interface SurebetLeg {
  bookmaker: string
  market: string
  selection: string
  odds: number
  line: number | null
  stake: number
  payout: number
  url: string | null
}

export interface ScannerMetrics {
  cycle_time_ms: number
  events_parsed: number
  surebets_found: number
  active_bookmakers: number
  failed_bookmakers: number
  cache_hit_rate: number
  memory_mb: number
  timestamp: string
}

export interface ScannerStatus {
  running: boolean
  cycle_count: number
  active_parsers: number
  last_metrics: ScannerMetrics | null
}

export interface AutoBetStatus {
  enabled: boolean
  running: boolean
  bets_placed_today: number
  bets_placed_total: number
  profit_today: number
  profit_total: number
  last_bet: string | null
  errors_today: number
  emergency_stopped: boolean
}

export interface ExecutionPlacementSummary {
  total: number
  pending: number
  placed: number
  settled: number
  cancelled: number
  errors: number
}

export interface AccountSessionSummary {
  total_bookmakers: number
  accounts_configured: number
  accounts_enabled: number
  disabled_accounts: number
  accounts_with_control_issues: number
  sessions_configured: number
  sessions_authenticated: number
  balances_cached: number
  ready_for_execution: number
  ready_for_dry_run: number
}

export interface ExecutionOverview {
  autobet_status: AutoBetStatus
  accounts: AccountSessionSummary
  recent_placements: ExecutionPlacementSummary
  ledger_placements: ExecutionPlacementSummary
  state_machine: ExecutionStateMachineMetadata
  generated_at: string
}

export interface ExecutionStatePhaseSummary {
  pending_placement: number
  confirmed_placement: number
  settled: number
  cancelled: number
  failed: number
}

export interface ExecutionStateSnapshotRecord {
  placement_id: string
  bookmaker: string
  phase: string
  placement_status: string
  sequence: number
  updated_at: string
  last_action: string
  last_error: string | null
}

export interface ExecutionStateMachineMetadata {
  total_snapshots: number
  total_transitions: number
  latest_snapshot_at: string | null
  latest_transition_at: string | null
  phases: ExecutionStatePhaseSummary
  recent_snapshots: ExecutionStateSnapshotRecord[]
}

export interface ExecutionStateTransitionRecord {
  placement_id: string
  bookmaker: string
  from_phase: string | null
  to_phase: string
  placement_status: string
  sequence: number
  action: string
  occurred_at: string
  error: string | null
}

export interface ExecutionBookmakerStateSummary {
  bookmaker: string
  total_snapshots: number
  phases: ExecutionStatePhaseSummary
  latest_snapshot_at: string | null
  latest_transition_at: string | null
  latest_error: string | null
}

export type BookmakerExecutionMode = 'NoOp' | 'Disabled' | 'DryRun' | 'Armed' | 'SemiRealReady' | 'Real'

export interface ExecutionStateReadinessSummary {
  total_bookmakers: number
  accounts_configured: number
  accounts_enabled: number
  auth_ready: number
  sessions_authenticated: number
  balances_cached: number
  dry_run_ready: number
  placement_ready: number
  approval_required: number
  submit_blocked_by_safe_mode: number
  operator_attention_required: number
}

export interface ExecutionBookmakerReadinessRecord {
  bookmaker: string
  account_configured: boolean
  account_enabled: boolean
  execution_mode: BookmakerExecutionMode | null
  requires_session: boolean
  auth_ready: boolean
  session_authenticated: boolean
  balance_cached: boolean
  dry_run_ready: boolean
  placement_ready: boolean
  approval_required: boolean
  submit_blocked_by_safe_mode: boolean
  operator_action: string | null
  blocking_reasons: string[]
}

export interface ExecutionStateAudit {
  total_snapshots: number
  total_transitions: number
  latest_snapshot_at: string | null
  latest_transition_at: string | null
  readiness: ExecutionStateReadinessSummary
  bookmaker_readiness: ExecutionBookmakerReadinessRecord[]
  bookmaker_summaries: ExecutionBookmakerStateSummary[]
  recent_transitions: ExecutionStateTransitionRecord[]
  generated_at: string
}

export interface ExecutionOperatorQueueItem {
  bookmaker: string
  severity: string
  priority_score: number
  execution_mode: BookmakerExecutionMode | null
  placement_ready: boolean
  approval_required: boolean
  submit_blocked_by_safe_mode: boolean
  session_stale: boolean
  balance_stale: boolean
  auth_snapshot_stale: boolean
  operator_action: string | null
  blocking_reasons: string[]
  persistence_warnings: string[]
  latest_error: string | null
  latest_transition_at: string | null
  latest_snapshot_at: string | null
}

export interface ExecutionOperatorQueueAudit {
  total_items: number
  critical_items: number
  warning_items: number
  items: ExecutionOperatorQueueItem[]
  generated_at: string
}

export type SemiAutoCouponStatus = 'awaiting_operator' | 'blocked' | 'applied_safe_mode'

export interface SemiAutoCouponLeg {
  bookmaker: string
  event_id: string
  market: string
  selection: string
  odds: number
  stake: number
  url: string | null
  preflight: {
    executable: boolean
    dry_run_ready: boolean
    placement_ready: boolean
    real_money_enabled: boolean
    approval_required: boolean
    submit_blocked_by_safe_mode: boolean
    validation: {
      decision: string
      adjusted_stake: number
      reasons: string[]
    }
  }
  receipt: {
    status: string
    mode: BookmakerExecutionMode
    message: string | null
    accepted_stake: number
    accepted_odds: number
    placed_at: string
  } | null
}

export interface SemiAutoCoupon {
  id: string
  surebet_id: string
  status: SemiAutoCouponStatus
  profit_percent: number
  total_stake: number
  league: string
  home_team: string
  away_team: string
  is_live: boolean
  operator_required: boolean
  safe_mode: boolean
  all_legs_ready: boolean
  blocking_reasons: string[]
  legs: SemiAutoCouponLeg[]
  created_at: string
  applied_at: string | null
}

export interface ExecutionLedgerPlacement {
  id: string
  bookmaker: string
  event: BackendEvent
  market: string
  selection: string
  odds: number
  stake: number
  status: string
  placed_at: string
  error: string | null
}

export interface ExecutionLedgerRecord {
  placement: ExecutionLedgerPlacement
  action: string
  recorded_at: string
}

export interface ExecutionLedgerAudit {
  total_entries: number
  unique_placements: number
  latest_recorded_at: string | null
  state_machine: ExecutionStateMachineMetadata
  recent_records: ExecutionLedgerRecord[]
  generated_at: string
}

export interface Bookmaker {
  name: string
  slug: string
  status: UiBookmakerStatus
  events: number
  odds: number
  last_update: string | null
  enabled?: boolean
  scan_supported?: boolean
  execution_supported?: boolean
  backend_status?: BackendBookmakerStatus
  notes?: string | null
}

export interface BookmakerAccountCapabilityMetadata {
  api_base_url: string | null
  planned_endpoints: string[]
  supports_read_only_session_sync: boolean
  supports_read_only_balance_refresh: boolean
  remote_balance_fetch_enabled: boolean
  notes: string[]
}

export interface BookmakerExecutionCapability {
  bookmaker: string
  supports_dry_run: boolean
  supports_balance_snapshot: boolean
  supports_bet_placement: boolean
  supports_real_money: boolean
  requires_session: boolean
  account_metadata: BookmakerAccountCapabilityMetadata
}

export interface BookmakerAccount {
  id: string
  bookmaker: string
  label: string
  currency: string
  enabled: boolean
  mode: string
  created_at: string
  last_used_at: string | null
}

export interface BookmakerSession {
  account_id: string
  bookmaker: string
  state: string
  token_hint: string | null
  last_synced_at: string
  expires_at: string | null
}

export interface AccountSessionMaterialResponse {
  source: string
  cookie_header_present: boolean
  authorization_header_present: boolean
  csrf_token_present: boolean
  user_agent_present: boolean
  extra_header_count: number
  imported_at: string
  redacted_hint: string
  persistence: string
}

export interface BookmakerBalanceSnapshot {
  account_id: string
  bookmaker: string
  currency: string
  total_balance: number
  available_balance: number
  exposure: number
  captured_at: string
}

export type BookmakerBalanceRefreshState = 'NoSession' | 'SessionNotAuthenticated' | 'AuthenticatedBalanceUnavailable' | 'CachedBalanceAvailable'

export interface BookmakerBalanceRefresh {
  account_id: string | null
  bookmaker: string
  state: BookmakerBalanceRefreshState
  session_status: string
  snapshot: BookmakerBalanceSnapshot | null
  detail: string | null
  checked_at: string
}

export interface AccountControlUpdate {
  enabled?: boolean
  armed?: boolean
}

export interface AccountSessionImportPayload {
  rawImport?: string
  cookieHeader?: string
  authorizationHeader?: string
  csrfToken?: string
  userAgent?: string
  availableBalance?: number
  expiresInHours?: number
}

export interface AccountAutomatedLoginPayload {
  login: string
  password: string
  loginUrl?: string
  availableBalance?: number
  waitTimeoutSecs?: number
}

export interface AccountAutomatedLoginSummary {
  bookmaker: string
  status: string
  authenticated: boolean
  login_url: string
  profile_dir: string
  storage_state_path: string
  cookie_count: number
  origin_count: number
  filled_login: boolean
  filled_password: boolean
  clicked_submit: boolean
  balance_text: string | null
  detail: string | null
  duration_secs: number
  checked_at: string
}

export interface AccountAutomatedLoginResponse {
  account: AccountStateResponse
  automation: AccountAutomatedLoginSummary
}

export interface AccountReadinessResponse {
  session_ready: boolean
  balance_ready: boolean
  dry_run_ready: boolean
  can_arm_safely: boolean
  placement_ready: boolean
  real_money_enabled: boolean
  rollout_gate_active: boolean
  approval_required: boolean
  submit_blocked_by_safe_mode: boolean
  operator_action: string | null
  blocking_reasons: string[]
}

export interface AccountStateResponse {
  bookmaker: string
  capability: BookmakerExecutionCapability
  account: BookmakerAccount | null
  session: BookmakerSession | null
  session_material: AccountSessionMaterialResponse | null
  balance: BookmakerBalanceSnapshot | null
  readiness: AccountReadinessResponse
  control_issues: string[]
}

export interface BankrollBookmakerBalance {
  bookmaker: string
  balance: number
  exposure: number
  available: number
  recommended_deposit: number
  recommended_withdraw: number
}

export interface BankrollState {
  total_budget: number
  bookmakers: BankrollBookmakerBalance[]
  total_exposure: number
  daily_profit: number
  daily_loss: number
  total_profit: number
  updated_at: string
}

export type DepositUrgency = 'Low' | 'Medium' | 'High' | 'low' | 'medium' | 'high'

export interface DepositAllocationTarget {
  bookmaker: string
  current_available: number
  target_available: number
  recommended_deposit: number
  deposit_gap: number
  urgency: DepositUrgency
  note: string
}

export interface DepositAllocationGuidance {
  total_budget_limit: number
  current_available_total: number
  target_per_bookmaker: number
  total_recommended_deposit: number
  targets: DepositAllocationTarget[]
}

export interface BankrollRecommendationsResponse {
  rebalance: BankrollBookmakerBalance[]
  deposit_guidance: DepositAllocationGuidance
}

export interface BackendBookmaker {
  slug: string
  name: string
  enabled?: boolean
  scan_supported?: boolean
  execution_supported?: boolean
  status?: BackendBookmakerStatus
  notes?: string | null
  id?: string
  url_live?: string
  priority?: number
}

export type BackendDiagnosticSeverity = 'pass' | 'warn' | 'fail' | 'info'
export type BackendParserReadinessStage = 'production' | 'rollout_ready' | 'diagnostic_only' | 'blocked'
export type BackendParserHealthStatus = 'Healthy' | 'Degraded' | 'Unhealthy' | 'CircuitOpen'

export interface ParserDiagnosticCheck {
  code: string
  severity: BackendDiagnosticSeverity
  message: string
}

export interface ParserReadiness {
  stage: BackendParserReadinessStage
  production_enabled: boolean
  self_check_available: boolean
  checks: ParserDiagnosticCheck[]
}

export interface ParserCoverage {
  slug: string
  name: string
  enabled: boolean
  scan_supported: boolean
  execution_supported: boolean
  status: BackendBookmakerStatus
  parser_type: string
  source: string
  notes: string | null
  readiness: ParserReadiness | null
}

export interface ParserHealth {
  bookmaker: string
  status: BackendParserHealthStatus
  last_success: string | null
  last_error: string | null
  consecutive_failures: number
  avg_response_time_ms: number
  events_parsed: number
  uptime_percent: number
  readiness: ParserReadiness | null
  diagnostics: ParserDiagnosticCheck[]
}

export type BackendNightlyKpiGateStatus = 'pass' | 'warn' | 'fail' | 'unavailable'
export type BackendBookmakerTriageBucket = 'ready' | 'unfinished' | 'disabled'

export interface ParserNightlyKpiGate {
  status: BackendNightlyKpiGateStatus
  checks: ParserDiagnosticCheck[]
}

export interface BookmakerStatusCatalogEntry {
  slug: string
  name: string
  enabled: boolean
  triage_bucket: BackendBookmakerTriageBucket
  readiness_stage: BackendParserReadinessStage
  production_enabled: boolean
  scan_supported: boolean
  execution_supported: boolean
  top_issue: string | null
  nightly_kpi_gate: ParserNightlyKpiGate
}

export interface BookmakerStatusCatalogSummary {
  ready: number
  unfinished: number
  disabled: number
  total: number
}

export interface BookmakerStatusCatalog {
  summary: BookmakerStatusCatalogSummary
  bookmakers: BookmakerStatusCatalogEntry[]
}

export interface CorridorOpportunity {
  id: string
  sport: string
  league: string
  home_team: string
  away_team: string
  market: string
  line_low: number
  line_high: number
  double_win_probability: number
  expected_roi: number
  legs: CorridorLeg[]
  detected_at: string
}

export interface BackendCorridorOpportunity {
  id: string
  sport: string
  league: string
  home_team: string
  away_team: string
  start_time: string | null
  is_live: boolean
  bookmaker_a: string
  bookmaker_b: string
  market: string
  line_a: number
  odds_a: number
  line_b: number
  odds_b: number
  corridor_size: number
  double_win_probability: number
  expected_roi: number
  detected_at: string
  ev_percent?: number
  scenarios?: Array<{
    probability: number
    both_win?: boolean
  }>
}

export interface BackendCollectionResponse<T> {
  total: number
  surebets?: T[]
  corridors?: T[]
}

export interface BackendEvent {
  id: string
  sport: string
  league: string
  home_team: string
  away_team: string
  start_time: string | null
  is_live: boolean
  bookmaker_slug: string
  raw_url: string | null
}

export interface ValueBet {
  id: string
  bookmaker: string
  event: BackendEvent
  market: string
  selection: string
  odds: number
  fair_odds: number
  edge_percent: number
  detected_at: string
}

export interface BackendGenerosityIndex {
  bookmaker: string
  sport: string
  avg_margin: number
  avg_odds: number
  best_odds_count: number
  total_events: number
  score: number
  updated_at: string
}

export type GenerosityIndex = BackendGenerosityIndex

export interface FreebetLifecycleLabelCount {
  label: string
  count: number
}

export interface FreebetLifecycleFundingGapLeader {
  bookmaker: string
  amount: number
}

export interface FreebetLifecycleSummary {
  total_bookmakers: number
  opportunities: number
  active_bonuses: number
  tracked_plans: number
  deposit_required_bookmakers: number
  blocked_states: number
  total_funding_gap: number
  largest_funding_gap: FreebetLifecycleFundingGapLeader | null
  discovered: number
  available: number
  qualified: number
  planned: number
  rollover_in_progress: number
  rollover_completed: number
  next_milestones: FreebetLifecycleLabelCount[]
  blockers: FreebetLifecycleLabelCount[]
  read_only_focuses: FreebetLifecycleLabelCount[]
  total_freebet_amount: number
  total_estimated_profit: number
  generated_at: string
}

export interface CorridorLeg {
  bookmaker: string
  selection: string
  odds: number
  line: number
}

export interface BackendExpressFork {
  id: string
  profit_percent: number
  total_stake: number
  legs: BackendExpressForkLeg[]
  detected_at: string
  verified: boolean
  risk_level: BackendExpressRiskLevel
}

export interface BackendExpressForkLeg {
  bookmaker: string
  event: BackendExpressForkEvent
  market: string
  selection: string
  odds: number
  stake: number
  is_express: boolean
  express_events: string[]
}

export interface BackendExpressForkEvent {
  id: string
  sport: string
  league: string
  home_team: string
  away_team: string
  start_time: string | null
  is_live: boolean
  bookmaker_slug: string
  raw_url: string | null
}

export type ExpressFork = BackendExpressFork
export type ExpressForkLeg = BackendExpressForkLeg
export type ExpressForkEvent = BackendExpressForkEvent

export type TabType = 'dashboard' | 'surebets' | 'corridors' | 'express' | 'operator' | 'accounts' | 'history' | 'settings'
