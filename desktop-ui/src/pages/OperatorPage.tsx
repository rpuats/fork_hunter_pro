import { useMemo, useState, useEffect } from 'react'
import { toast } from 'sonner'
import { motion } from 'framer-motion'
import { Activity, AlertTriangle, CheckCircle2, Clock3, PauseCircle, PlayCircle, Search, ShieldAlert, ShieldCheck, ShieldX, Siren, TimerReset, Wallet } from 'lucide-react'
import { CompactSignalOverlay } from '../components/CompactSignalOverlay'
import type { AccountStateResponse, BackendParserHealthStatus, BackendParserReadinessStage, Bookmaker, BookmakerExecutionMode, BookmakerStatusCatalog, ExecutionBookmakerReadinessRecord, ExecutionLedgerAudit, ExecutionOperatorQueueAudit, ExecutionOverview, ExecutionStateAudit, ExecutionStateReadinessSummary, ExecutionStateSnapshotRecord, FreebetLifecycleSummary, ParserCoverage, ParserHealth, SemiAutoCoupon } from '../types'

interface OperatorPageProps {
  executionOverview: ExecutionOverview | null
  executionLedger: ExecutionLedgerAudit | null
  executionState: ExecutionStateAudit | null
  executionOperatorQueue: ExecutionOperatorQueueAudit | null
  semiAutoCoupons: SemiAutoCoupon[]
  onConfirmSemiAutoCoupon: (couponId: string) => Promise<SemiAutoCoupon | null>
  parserCoverage: ParserCoverage[]
  parserHealth: ParserHealth[]
  bookmakers: Bookmaker[]
  bookmakerStatusCatalog: BookmakerStatusCatalog | null
  accountStates: AccountStateResponse[]
  freebetSummary: FreebetLifecycleSummary | null
  onOpenAccount: (bookmaker: string) => void
}

type ReadinessFilter = 'all' | 'attention' | 'execution' | 'blocked'
type LedgerFilter = 'all' | 'errors' | 'pending' | 'settled' | 'active'
type LedgerSort = 'priority' | 'newest' | 'oldest' | 'stake' | 'odds'

interface TimelineItem {
  id: string
  recordedAt: string
  title: string
  subtitle: string
  meta: string
  status: string
  stake: number | null
  odds: number | null
  error: string | null
  bookmaker: string
  action: string
  source: 'ledger' | 'state'
  priority: number
}

interface BookmakerHotspot {
  key: string
  name: string
  score: number
  tone: 'danger' | 'warning' | 'info' | 'success'
  parserStatus: BackendParserHealthStatus | null
  readinessStage: BackendParserReadinessStage
  ledgerErrors: number
  pendingPlacements: number
  accountBlocked: boolean
  safeModeBlocked: boolean
  approvalRequired: boolean
  latestAt: string | null
  reasons: string[]
  timelineBookmaker: string | null
}

const container = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: { staggerChildren: 0.05 },
  },
}

const item = {
  hidden: { opacity: 0, y: 20 },
  show: { opacity: 1, y: 0, transition: { duration: 0.3 } },
}

function formatDateTime(value: string | null) {
  if (!value) return '—'

  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '—'

  return date.toLocaleString('ru-RU', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function formatCurrency(value: number) {
  return `${value >= 0 ? '+' : ''}${value.toLocaleString('ru-RU', { maximumFractionDigits: 2 })} RUB`
}

function formatPercent(value: number) {
  return `${value.toFixed(0)}%`
}

function formatRelativeAge(value: string | null) {
  if (!value) return 'no data'

  const timestamp = new Date(value).getTime()
  if (Number.isNaN(timestamp)) return 'no data'

  const diffMs = Math.max(Date.now() - timestamp, 0)
  const diffMinutes = Math.floor(diffMs / 60000)

  if (diffMinutes < 1) return '<1 min ago'
  if (diffMinutes < 60) return `${diffMinutes} min ago`

  const diffHours = Math.floor(diffMinutes / 60)
  if (diffHours < 24) return `${diffHours} h ago`

  const diffDays = Math.floor(diffHours / 24)
  return `${diffDays} d ago`
}

function toTitleCase(value: string) {
  return value
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (char) => char.toUpperCase())
}

function normalizeKey(value: string) {
  return value.trim().toLowerCase().replace(/\s+/g, '-')
}

function healthBadgeClass(status: BackendParserHealthStatus | null) {
  switch (status) {
    case 'Healthy':
      return 'badge-success'
    case 'Degraded':
      return 'badge-warning'
    case 'Unhealthy':
    case 'CircuitOpen':
      return 'badge-danger'
    default:
      return 'badge-info'
  }
}

function readinessBadgeClass(stage: BackendParserReadinessStage) {
  switch (stage) {
    case 'production':
      return 'badge-success'
    case 'rollout_ready':
      return 'badge-info'
    case 'diagnostic_only':
      return 'badge-warning'
    case 'blocked':
      return 'badge-danger'
  }
}

function formatExecutionMode(mode: BookmakerExecutionMode | null | undefined) {
  if (!mode) return 'No account'
  return mode.replace(/([a-z])([A-Z])/g, '$1 $2')
}

export function OperatorPage({ executionOverview, executionLedger, executionState, executionOperatorQueue, semiAutoCoupons, onConfirmSemiAutoCoupon, parserCoverage, parserHealth, bookmakers, bookmakerStatusCatalog, accountStates, freebetSummary, onOpenAccount }: OperatorPageProps) {
  // Local UI mode for automation: auto or semi-auto (MVP; backend not wired yet)
  const [autoMode, setAutoMode] = useState<string>(() => {
    const saved = typeof window !== 'undefined' ? localStorage.getItem('auto_mode') : null
    return saved ?? 'auto'
  })

  useEffect(() => {
    localStorage.setItem('auto_mode', autoMode)
  }, [autoMode])
  const [readinessFilter, setReadinessFilter] = useState<ReadinessFilter>('attention')
  const [ledgerFilter, setLedgerFilter] = useState<LedgerFilter>('errors')
  const [ledgerSort, setLedgerSort] = useState<LedgerSort>('priority')
  const [ledgerBookmaker, setLedgerBookmaker] = useState<string>('all')
  const [ledgerQuery, setLedgerQuery] = useState('')
  const [confirmingCouponId, setConfirmingCouponId] = useState<string | null>(null)
  const executionStatus = executionOverview?.autobet_status ?? null
  const accounts = executionOverview?.accounts ?? null
  const recentPlacements = executionOverview?.recent_placements ?? null
  const ledgerPlacements = executionOverview?.ledger_placements ?? null
  const stateMachine = executionLedger?.state_machine ?? executionOverview?.state_machine ?? null
  const stateDiagnostics = executionState ?? null
  const stateMachineSnapshot = executionState
    ? {
        total_snapshots: executionState.total_snapshots,
        total_transitions: executionState.total_transitions,
        latest_snapshot_at: executionState.latest_snapshot_at,
        latest_transition_at: executionState.latest_transition_at,
        phases: executionState.bookmaker_summaries.reduce((summary, entry) => ({
          pending_placement: summary.pending_placement + entry.phases.pending_placement,
          confirmed_placement: summary.confirmed_placement + entry.phases.confirmed_placement,
          settled: summary.settled + entry.phases.settled,
          cancelled: summary.cancelled + entry.phases.cancelled,
          failed: summary.failed + entry.phases.failed,
        }), { pending_placement: 0, confirmed_placement: 0, settled: 0, cancelled: 0, failed: 0 }),
        recent_snapshots: [],
      }
    : stateMachine

  const executionTone = executionStatus?.emergency_stopped
    ? { label: 'Emergency stop', badge: 'badge-danger', icon: ShieldX, accent: 'var(--accent-red)' }
    : executionStatus?.running
      ? { label: 'Autobet running', badge: 'badge-success', icon: PlayCircle, accent: 'var(--accent-green)' }
      : executionStatus?.enabled
        ? { label: 'Standby / armed', badge: 'badge-warning', icon: PauseCircle, accent: 'var(--accent-yellow)' }
        : { label: 'Disabled', badge: 'badge-info', icon: PauseCircle, accent: 'var(--accent-blue)' }

  const readinessRows = useMemo(() => {
    const coverageBySlug = new Map(parserCoverage.map((entry) => [entry.slug, entry]))
    const healthByBookmaker = new Map(parserHealth.map((entry) => [normalizeKey(entry.bookmaker), entry]))

    return bookmakers
      .map((bookmaker) => {
        const coverage = coverageBySlug.get(bookmaker.slug)
        const readiness = coverage?.readiness
        const health = healthByBookmaker.get(normalizeKey(bookmaker.slug)) ?? healthByBookmaker.get(normalizeKey(bookmaker.name))
        const mode = coverage?.execution_supported
          ? readiness?.production_enabled ? 'armed path' : 'dry-run path'
          : bookmaker.execution_supported ? 'contract only' : 'scan only'
        const stage = readiness?.stage ?? (coverage?.enabled || bookmaker.enabled ? 'production' : 'blocked')
        const checks = readiness?.checks ?? []
        const failingChecks = checks.filter((check) => check.severity === 'warn' || check.severity === 'fail')
        const healthStatus = health?.status ?? null
        const hasHealthIssue = healthStatus === 'Unhealthy' || healthStatus === 'CircuitOpen'
        const hasAttention = stage === 'blocked' || failingChecks.length > 0 || hasHealthIssue

        return {
          slug: bookmaker.slug,
          name: coverage?.name ?? bookmaker.name,
          mode,
          backendStatus: coverage?.status ?? bookmaker.backend_status ?? 'disabled',
          stage,
          checks,
          failingChecks,
          notes: coverage?.notes ?? bookmaker.notes ?? null,
          executionSupported: coverage?.execution_supported ?? bookmaker.execution_supported ?? false,
          scanSupported: coverage?.scan_supported ?? bookmaker.scan_supported ?? false,
          healthStatus,
          consecutiveFailures: health?.consecutive_failures ?? 0,
          uptimePercent: health?.uptime_percent ?? null,
          avgResponseTimeMs: health?.avg_response_time_ms ?? null,
          lastError: health?.last_error ?? null,
          hasAttention,
          group: hasAttention ? 'attention' : (coverage?.execution_supported ?? bookmaker.execution_supported ?? false) ? 'execution' : 'watchlist',
        }
      })
      .sort((a, b) => Number(b.hasAttention) - Number(a.hasAttention) || Number(b.executionSupported) - Number(a.executionSupported) || a.name.localeCompare(b.name))
  }, [bookmakers, parserCoverage, parserHealth])

  const readinessSummary = useMemo(() => ({
    executionReady: readinessRows.filter((entry) => entry.executionSupported).length,
    production: readinessRows.filter((entry) => entry.stage === 'production').length,
    blocked: readinessRows.filter((entry) => entry.stage === 'blocked').length,
    warnings: readinessRows.filter((entry) => entry.checks.some((check) => check.severity === 'warn' || check.severity === 'fail')).length,
  }), [readinessRows])
  const executionReadinessSummary = useMemo<ExecutionStateReadinessSummary>(() => {
    if (stateDiagnostics?.readiness) return stateDiagnostics.readiness

    return {
      total_bookmakers: accounts?.total_bookmakers ?? accountStates.length,
      accounts_configured: accounts?.accounts_configured ?? accountStates.filter((account) => account.account).length,
      accounts_enabled: accounts?.accounts_enabled ?? accountStates.filter((account) => account.account?.enabled).length,
      auth_ready: accountStates.filter((account) => account.readiness.session_ready).length,
      sessions_authenticated: accounts?.sessions_authenticated ?? accountStates.filter((account) => account.readiness.session_ready).length,
      balances_cached: accounts?.balances_cached ?? accountStates.filter((account) => account.balance).length,
      dry_run_ready: accounts?.ready_for_dry_run ?? accountStates.filter((account) => account.readiness.dry_run_ready).length,
      placement_ready: accounts?.ready_for_execution ?? accountStates.filter((account) => account.readiness.placement_ready).length,
      approval_required: accountStates.filter((account) => account.readiness.approval_required).length,
      submit_blocked_by_safe_mode: accountStates.filter((account) => account.readiness.submit_blocked_by_safe_mode).length,
      operator_attention_required: accountStates.filter((account) => account.readiness.approval_required || Boolean(account.readiness.operator_action)).length,
    }
  }, [accountStates, accounts, stateDiagnostics])
  const executionReadinessRows = useMemo<ExecutionBookmakerReadinessRecord[]>(() => {
    if (stateDiagnostics?.bookmaker_readiness?.length) return stateDiagnostics.bookmaker_readiness

    return accountStates.map((account) => ({
      bookmaker: account.bookmaker,
      account_configured: Boolean(account.account),
      account_enabled: Boolean(account.account?.enabled),
      execution_mode: (account.account?.mode as BookmakerExecutionMode | undefined) ?? null,
      requires_session: account.capability.requires_session,
      auth_ready: account.readiness.session_ready,
      session_authenticated: account.readiness.session_ready,
      balance_cached: Boolean(account.balance),
      dry_run_ready: account.readiness.dry_run_ready,
      placement_ready: account.readiness.placement_ready,
      approval_required: account.readiness.approval_required,
      submit_blocked_by_safe_mode: account.readiness.submit_blocked_by_safe_mode,
      operator_action: account.readiness.operator_action,
      blocking_reasons: account.readiness.blocking_reasons,
    }))
  }, [accountStates, stateDiagnostics])
  const authReadinessRate = executionReadinessSummary.total_bookmakers > 0
    ? (executionReadinessSummary.auth_ready / executionReadinessSummary.total_bookmakers) * 100
    : 0
  const liveAuthRate = executionReadinessSummary.total_bookmakers > 0
    ? (executionReadinessSummary.sessions_authenticated / executionReadinessSummary.total_bookmakers) * 100
    : 0
  const executionPlacementRate = executionReadinessSummary.total_bookmakers > 0
    ? (executionReadinessSummary.placement_ready / executionReadinessSummary.total_bookmakers) * 100
    : 0

  const parserHealthSummary = useMemo(() => ({
    healthy: parserHealth.filter((entry) => entry.status === 'Healthy').length,
    degraded: parserHealth.filter((entry) => entry.status === 'Degraded').length,
    incidents: parserHealth.filter((entry) => entry.status === 'Unhealthy' || entry.status === 'CircuitOpen').length,
  }), [parserHealth])
  const bookmakerCatalogCards = useMemo(() => {
    if (!bookmakerStatusCatalog) return []

    const summary = bookmakerStatusCatalog.summary
    const hottest = bookmakerStatusCatalog.bookmakers.find((entry) => entry.triage_bucket === 'unfinished' && entry.top_issue)
    const nightlyWarn = bookmakerStatusCatalog.bookmakers.filter((entry) => entry.nightly_kpi_gate.status === 'warn' || entry.nightly_kpi_gate.status === 'fail').length

    return [
      {
        key: 'ready',
        label: 'Ready now',
        value: `${summary.ready}/${summary.total}`,
        detail: 'Bookmakers already classified as ready/rollout-capable by the status catalog.',
        badge: 'badge-success',
      },
      {
        key: 'unfinished',
        label: 'Unfinished',
        value: `${summary.unfinished}`,
        detail: hottest?.top_issue ?? 'Top unfinished bookmakers now have explicit readiness or blocker truth.',
        badge: 'badge-warning',
      },
      {
        key: 'nightly',
        label: 'Nightly gate',
        value: nightlyWarn > 0 ? `${nightlyWarn} warn/fail` : 'Passing',
        detail: nightlyWarn > 0
          ? 'Nightly KPI gate still has bookmakers below promotion confidence.'
          : 'Nightly KPI signals are currently aligned with promotion surfaces.',
        badge: nightlyWarn > 0 ? 'badge-warning' : 'badge-success',
      },
    ]
  }, [bookmakerStatusCatalog])

  const timelineItems = useMemo<TimelineItem[]>(() => {
    if (executionLedger?.recent_records.length) {
      return executionLedger.recent_records.map((record) => ({
        id: `${record.placement.id}-${record.recorded_at}-${record.action}`,
        recordedAt: record.recorded_at,
        title: `${record.placement.bookmaker} • ${record.action}`,
        subtitle: `${record.placement.event.home_team} vs ${record.placement.event.away_team}`,
        meta: `${record.placement.market} / ${record.placement.selection}`,
        status: record.placement.status,
        stake: record.placement.stake,
        odds: record.placement.odds,
        error: record.placement.error,
        bookmaker: record.placement.bookmaker,
        action: record.action,
        source: 'ledger',
        priority: record.placement.error
          ? 100
          : record.placement.status.toLowerCase().includes('pending')
            ? 70
            : record.placement.status.toLowerCase().includes('cancel')
              ? 65
              : record.placement.status.toLowerCase().includes('settled')
                ? 20
                : 40,
      }))
    }

    return (stateMachine?.recent_snapshots ?? []).map((snapshot: ExecutionStateSnapshotRecord) => ({
      id: `${snapshot.placement_id}-${snapshot.sequence}`,
      recordedAt: snapshot.updated_at,
      title: `${snapshot.bookmaker} • ${toTitleCase(snapshot.phase)}`,
      subtitle: `Action: ${toTitleCase(snapshot.last_action)}`,
      meta: `State ${toTitleCase(snapshot.placement_status)} / seq ${snapshot.sequence}`,
      status: snapshot.placement_status,
      stake: null,
      odds: null,
      error: snapshot.last_error,
      bookmaker: snapshot.bookmaker,
      action: snapshot.last_action,
      source: 'state',
      priority: snapshot.last_error
        ? 95
        : snapshot.phase.toLowerCase().includes('failed') || snapshot.placement_status.toLowerCase().includes('failed')
          ? 85
          : snapshot.phase.toLowerCase().includes('pending') || snapshot.placement_status.toLowerCase().includes('pending')
            ? 60
            : snapshot.phase.toLowerCase().includes('settled') || snapshot.placement_status.toLowerCase().includes('settled')
              ? 20
              : 35,
    }))
  }, [executionLedger, stateMachine])

  const ledgerBookmakerOptions = useMemo(() => {
    return Array.from(new Set(timelineItems.map((entry) => entry.bookmaker).filter(Boolean))).sort((a, b) => a.localeCompare(b))
  }, [timelineItems])

  const ledgerSummary = useMemo(() => ({
    errors: timelineItems.filter((entry) => Boolean(entry.error)).length,
    pending: timelineItems.filter((entry) => entry.status.toLowerCase().includes('pending')).length,
    settled: timelineItems.filter((entry) => entry.status.toLowerCase().includes('settled')).length,
    active: timelineItems.filter((entry) => !entry.error && !entry.status.toLowerCase().includes('settled')).length,
  }), [timelineItems])

  const filteredTimelineItems = useMemo(() => {
    const normalizedQuery = ledgerQuery.trim().toLowerCase()

    return timelineItems
      .filter((entry) => {
        if (ledgerBookmaker !== 'all' && entry.bookmaker !== ledgerBookmaker) return false

        if (ledgerFilter === 'errors' && !entry.error) return false
        if (ledgerFilter === 'pending' && !entry.status.toLowerCase().includes('pending')) return false
        if (ledgerFilter === 'settled' && !entry.status.toLowerCase().includes('settled')) return false
        if (ledgerFilter === 'active' && (entry.error || entry.status.toLowerCase().includes('settled'))) return false

        if (!normalizedQuery) return true

        const haystack = `${entry.title} ${entry.subtitle} ${entry.meta} ${entry.error ?? ''} ${entry.action} ${entry.bookmaker}`.toLowerCase()
        return haystack.includes(normalizedQuery)
      })
      .sort((a, b) => {
        const timeA = new Date(a.recordedAt).getTime()
        const timeB = new Date(b.recordedAt).getTime()

        switch (ledgerSort) {
          case 'oldest':
            return timeA - timeB
          case 'stake':
            return (b.stake ?? -1) - (a.stake ?? -1) || timeB - timeA
          case 'odds':
            return (b.odds ?? -1) - (a.odds ?? -1) || timeB - timeA
          case 'newest':
            return timeB - timeA
          default:
            return b.priority - a.priority || timeB - timeA
        }
      })
  }, [ledgerBookmaker, ledgerFilter, ledgerQuery, ledgerSort, timelineItems])

  const triageQueue = useMemo(() => filteredTimelineItems.slice(0, 4), [filteredTimelineItems])

  const readyRate = accounts && accounts.total_bookmakers > 0
    ? (accounts.ready_for_execution / accounts.total_bookmakers) * 100
    : 0
  const authRate = accounts && accounts.sessions_configured > 0
    ? (accounts.sessions_authenticated / accounts.sessions_configured) * 100
    : 0
  const ledgerErrorRate = ledgerPlacements && ledgerPlacements.total > 0
    ? (ledgerPlacements.errors / ledgerPlacements.total) * 100
    : 0

  const attentionCards = useMemo(() => {
    const cards: Array<{ key: string, title: string, detail: string, badge: string }> = []

    if (executionStatus?.emergency_stopped) {
      cards.push({
        key: 'emergency-stop',
        title: 'Execution остановлен аварийно',
        detail: 'Новые placement actions заблокированы до ручного восстановления backend-процесса.',
        badge: 'badge-danger',
      })
    }

    if ((accounts?.accounts_with_control_issues ?? 0) > 0) {
      cards.push({
        key: 'control-issues',
        title: `${accounts?.accounts_with_control_issues ?? 0} аккаунтов с control issues`,
        detail: `Ready for execution: ${accounts?.ready_for_execution ?? 0} из ${accounts?.total_bookmakers ?? readinessRows.length}.`,
        badge: 'badge-warning',
      })
    }

    if (parserHealthSummary.incidents > 0 || parserHealthSummary.degraded > 0) {
      cards.push({
        key: 'parser-health',
        title: `Parser incidents: ${parserHealthSummary.incidents} critical / ${parserHealthSummary.degraded} degraded`,
        detail: 'Используй health snapshot для быстрой сверки, почему execution-ready account ушёл в риск.',
        badge: parserHealthSummary.incidents > 0 ? 'badge-danger' : 'badge-warning',
      })
    }

    if (ledgerPlacements && ledgerPlacements.errors > 0) {
      cards.push({
        key: 'ledger-errors',
        title: `Ledger errors ${ledgerPlacements.errors} из ${ledgerPlacements.total}`,
        detail: `Текущий error rate ${formatPercent(ledgerErrorRate)}; timeline ниже уже подсвечивает problem placements.`,
        badge: 'badge-danger',
      })
    }

    return cards.slice(0, 4)
  }, [accounts, executionStatus, ledgerErrorRate, ledgerPlacements, parserHealthSummary, readinessRows.length])

  const filteredReadinessRows = useMemo(() => {
    switch (readinessFilter) {
      case 'attention':
        return readinessRows.filter((entry) => entry.hasAttention)
      case 'execution':
        return readinessRows.filter((entry) => entry.executionSupported)
      case 'blocked':
        return readinessRows.filter((entry) => entry.stage === 'blocked')
      default:
        return readinessRows
    }
  }, [readinessFilter, readinessRows])

  const readinessGroups = useMemo(() => ([
    {
      key: 'attention',
      title: 'Needs attention',
      description: 'Blocked readiness, warn/fail checks, unhealthy parser health.',
      rows: filteredReadinessRows.filter((entry) => entry.group === 'attention'),
    },
    {
      key: 'execution',
      title: 'Execution-ready lane',
      description: 'Execution contract exposed and no immediate incidents in GET snapshots.',
      rows: filteredReadinessRows.filter((entry) => entry.group === 'execution'),
    },
    {
      key: 'watchlist',
      title: 'Scan / watchlist lane',
      description: 'Safe to monitor, but not part of the current execution path.',
      rows: filteredReadinessRows.filter((entry) => entry.group === 'watchlist'),
    },
  ]), [filteredReadinessRows])

  const ToneIcon = executionTone.icon
  const transitionRows = useMemo(() => (stateDiagnostics?.recent_transitions ?? []).slice(0, 8), [stateDiagnostics])
  const authSurfaceRows = useMemo(() => {
    return [...executionReadinessRows]
      .map((entry) => {
        const issues = [
          ...(entry.operator_action ? [entry.operator_action] : []),
          ...entry.blocking_reasons,
        ]
        const uniqueIssues = Array.from(new Set(issues.filter(Boolean)))
        const priority =
          (entry.submit_blocked_by_safe_mode ? 90 : 0)
          + (entry.approval_required ? 50 : 0)
          + (!entry.account_configured ? 42 : 0)
          + (!entry.account_enabled ? 24 : 0)
          + (entry.requires_session && !entry.auth_ready ? 36 : 0)
          + (entry.requires_session && !entry.session_authenticated ? 22 : 0)
          + (!entry.balance_cached ? 18 : 0)
          + (!entry.placement_ready ? 24 : 0)
          + Math.min(uniqueIssues.length * 6, 18)

        return {
          ...entry,
          issues: uniqueIssues,
          priority,
          tone: entry.submit_blocked_by_safe_mode
            ? 'danger'
            : entry.approval_required || !entry.placement_ready || (entry.requires_session && !entry.session_authenticated)
              ? 'warning'
              : 'success',
        }
      })
      .sort((a, b) => b.priority - a.priority || a.bookmaker.localeCompare(b.bookmaker))
      .slice(0, 8)
  }, [executionReadinessRows])
  const operatorQueueRows = executionOperatorQueue?.items.slice(0, 8) ?? []
  const bookmakerStateRows = useMemo(() => {
    return [...(stateDiagnostics?.bookmaker_summaries ?? [])]
      .sort((a, b) => Number(Boolean(b.latest_error)) - Number(Boolean(a.latest_error)) || b.total_snapshots - a.total_snapshots || a.bookmaker.localeCompare(b.bookmaker))
      .slice(0, 6)
  }, [stateDiagnostics])
  const stateActivityAt = stateDiagnostics?.latest_transition_at
    ?? stateDiagnostics?.latest_snapshot_at
    ?? executionLedger?.latest_recorded_at
    ?? executionOverview?.generated_at
    ?? null
  const stateFreshness = useMemo(() => {
    if (!stateActivityAt) {
      return {
        label: 'No state signal',
        badge: 'badge-info',
        detail: 'Waiting for execution state snapshots.',
      }
    }

    const stateActivityTimestamp = new Date(stateActivityAt).getTime()
    if (Number.isNaN(stateActivityTimestamp)) {
      return {
        label: 'No state signal',
        badge: 'badge-info',
        detail: 'Execution state timestamp is not parseable.',
      }
    }

    const ageMs = Math.max(Date.now() - stateActivityTimestamp, 0)
    if (ageMs <= 2 * 60 * 1000) {
      return {
        label: 'Fresh snapshot',
        badge: 'badge-success',
        detail: `${formatRelativeAge(stateActivityAt)} from latest state activity.`,
      }
    }

    if (ageMs <= 10 * 60 * 1000) {
      return {
        label: 'Delayed snapshot',
        badge: 'badge-warning',
        detail: `${formatRelativeAge(stateActivityAt)}; verify poll cadence before triage.`,
      }
    }

    return {
      label: 'Stale snapshot',
      badge: 'badge-danger',
      detail: `${formatRelativeAge(stateActivityAt)}; timeline may no longer reflect live execution.`,
    }
  }, [stateActivityAt])
  const operatorBrief = useMemo(() => {
    const topTriage = triageQueue[0] ?? null
    const topReadiness = readinessRows.find((entry) => entry.hasAttention) ?? null
    const totalHotspots = attentionCards.length + parserHealthSummary.incidents + readinessSummary.blocked + ledgerSummary.errors

    return {
      tone: executionStatus?.emergency_stopped || ledgerSummary.errors > 0 || parserHealthSummary.incidents > 0
        ? 'danger'
        : readinessSummary.warnings > 0 || stateFreshness.badge === 'badge-warning'
          ? 'warning'
          : 'success',
      summary: totalHotspots > 0
        ? `${totalHotspots} active operator signals across execution, readiness and parser health. ${topTriage ? `Current front-of-queue is ${topTriage.bookmaker} with ${topTriage.error ? 'an execution error' : toTitleCase(topTriage.status)}.` : 'Snapshot is ready for triage.'}`
        : 'No blocking signals surfaced in the current execution snapshot; use the cards below for quick verification.',
      pills: [
        { label: 'freshness', value: stateFreshness.label, tone: stateFreshness.badge === 'badge-danger' ? 'danger' : stateFreshness.badge === 'badge-warning' ? 'warning' : 'success' },
        { label: 'ledger', value: `${ledgerSummary.errors} errors`, tone: ledgerSummary.errors > 0 ? 'danger' : 'success' },
        { label: 'readiness', value: `${readinessSummary.blocked} blocked`, tone: readinessSummary.blocked > 0 ? 'warning' : 'success' },
        { label: 'parser', value: `${parserHealthSummary.incidents} incidents`, tone: parserHealthSummary.incidents > 0 ? 'danger' : parserHealthSummary.degraded > 0 ? 'warning' : 'success' },
      ],
      actions: [
        topTriage
          ? `${topTriage.bookmaker}: ${topTriage.error ?? `${toTitleCase(topTriage.status)} state on ${topTriage.meta}.`}`
          : 'No execution records in the current triage queue.',
        topReadiness
          ? `${topReadiness.name}: ${topReadiness.failingChecks[0]?.message ?? topReadiness.lastError ?? topReadiness.notes ?? 'Readiness attention is visible on this lane.'}`
          : 'No parser readiness blockers are currently surfaced.',
      ],
    } as const
  }, [attentionCards.length, executionStatus?.emergency_stopped, ledgerSummary.errors, parserHealthSummary.degraded, parserHealthSummary.incidents, readinessRows, readinessSummary.blocked, readinessSummary.warnings, stateFreshness.badge, stateFreshness.label, triageQueue])
  const operatorActionQueue = useMemo(() => {
    return accountStates
      .map((account) => {
        const reasons = [
          ...(account.readiness.operator_action ? [account.readiness.operator_action] : []),
          ...account.readiness.blocking_reasons,
          ...account.control_issues,
        ]
        const uniqueReasons = Array.from(new Set(reasons.filter(Boolean)))
        const priority =
          (account.readiness.submit_blocked_by_safe_mode ? 90 : 0)
          + (account.readiness.approval_required ? 70 : 0)
          + (!account.readiness.session_ready ? 55 : 0)
          + (!account.readiness.balance_ready ? 35 : 0)
          + (!account.readiness.placement_ready ? 25 : 0)
          + Math.min(account.control_issues.length * 8, 24)
          + Math.min(account.readiness.blocking_reasons.length * 6, 18)

        return {
          bookmaker: account.bookmaker,
          mode: account.account?.mode ?? 'no account',
          sessionState: account.session?.state ?? 'missing',
          availableBalance: account.balance?.available_balance ?? null,
          reasons: uniqueReasons,
          priority,
          tone: account.readiness.submit_blocked_by_safe_mode
            ? 'danger'
            : account.readiness.approval_required || !account.readiness.session_ready || !account.readiness.balance_ready
              ? 'warning'
              : 'info',
        }
      })
      .filter((entry) => entry.reasons.length > 0)
      .sort((a, b) => b.priority - a.priority || a.bookmaker.localeCompare(b.bookmaker))
      .slice(0, 6)
  }, [accountStates])
  const operatorActionSummary = useMemo(() => ({
    safeMode: accountStates.filter((account) => account.readiness.submit_blocked_by_safe_mode).length,
    approval: accountStates.filter((account) => account.readiness.approval_required).length,
    sessionRestore: accountStates.filter((account) => !account.readiness.session_ready).length,
    balanceRefresh: accountStates.filter((account) => !account.readiness.balance_ready).length,
  }), [accountStates])
  const bookmakerHotspots = useMemo<BookmakerHotspot[]>(() => {
    const healthByBookmaker = new Map(parserHealth.map((entry) => [normalizeKey(entry.bookmaker), entry]))
    const accountByBookmaker = new Map(accountStates.map((entry) => [normalizeKey(entry.bookmaker), entry]))
    const readinessByBookmaker = new Map(readinessRows.flatMap((entry) => ([
      [normalizeKey(entry.slug), entry] as const,
      [normalizeKey(entry.name), entry] as const,
    ])))
    const stateByBookmaker = new Map((stateDiagnostics?.bookmaker_summaries ?? []).map((entry) => [normalizeKey(entry.bookmaker), entry]))
    const ledgerByBookmaker = timelineItems.reduce((summary, entry) => {
      const key = normalizeKey(entry.bookmaker)
      const current = summary.get(key) ?? { errors: 0, pending: 0, latestAt: null as string | null, bookmaker: entry.bookmaker }

      if (entry.error) current.errors += 1
      if (entry.status.toLowerCase().includes('pending')) current.pending += 1
      current.bookmaker = current.bookmaker || entry.bookmaker

      const currentTimestamp = current.latestAt ? new Date(current.latestAt).getTime() : 0
      const entryTimestamp = new Date(entry.recordedAt).getTime()
      if (!current.latestAt || (!Number.isNaN(entryTimestamp) && entryTimestamp > currentTimestamp)) {
        current.latestAt = entry.recordedAt
      }

      summary.set(key, current)
      return summary
    }, new Map<string, { errors: number, pending: number, latestAt: string | null, bookmaker: string | null }>())

    const allKeys = new Set<string>([
      ...readinessRows.map((entry) => normalizeKey(entry.slug)),
      ...parserHealth.map((entry) => normalizeKey(entry.bookmaker)),
      ...accountStates.map((entry) => normalizeKey(entry.bookmaker)),
      ...timelineItems.map((entry) => normalizeKey(entry.bookmaker)),
      ...(stateDiagnostics?.bookmaker_summaries ?? []).map((entry) => normalizeKey(entry.bookmaker)),
    ])

    return [...allKeys]
      .map((key) => {
        const readiness = readinessByBookmaker.get(key) ?? null
        const health = healthByBookmaker.get(key) ?? null
        const account = accountByBookmaker.get(key) ?? null
        const ledger = ledgerByBookmaker.get(key) ?? null
        const state = stateByBookmaker.get(key) ?? null
        const reasons: string[] = []
        let score = 0

        if (health?.status === 'Unhealthy' || health?.status === 'CircuitOpen') {
          score += 34
          reasons.push(health.last_error ?? `${health.status} parser runtime`)
        } else if (health?.status === 'Degraded') {
          score += 16
          reasons.push(health.last_error ?? 'parser runtime degraded')
        }

        if (readiness?.stage === 'blocked') {
          score += 24
          reasons.push(readiness.failingChecks[0]?.message ?? readiness.notes ?? 'parser readiness blocked')
        } else if (readiness?.stage === 'diagnostic_only') {
          score += 12
          reasons.push(readiness.failingChecks[0]?.message ?? 'parser is still diagnostic-only')
        } else if (readiness?.failingChecks.length) {
          score += Math.min(readiness.failingChecks.length * 6, 18)
          reasons.push(readiness.failingChecks[0]?.message)
        }

        if (account && !account.readiness.placement_ready) {
          score += 24
          reasons.push(account.readiness.operator_action ?? account.readiness.blocking_reasons[0] ?? 'account not ready for placement')
        }

        if (account?.readiness.submit_blocked_by_safe_mode) {
          score += 18
          reasons.push('safe mode blocks submit')
        }

        if (account?.readiness.approval_required) {
          score += 8
          reasons.push('approval required before placement')
        }

        if (account && !account.readiness.session_ready) {
          score += 10
          reasons.push('session restore needed')
        }

        if (account && !account.readiness.balance_ready) {
          score += 6
          reasons.push('balance snapshot stale or missing')
        }

        if (account?.control_issues.length) {
          score += Math.min(account.control_issues.length * 6, 18)
          reasons.push(account.control_issues[0])
        }

        if ((ledger?.errors ?? 0) > 0) {
          score += Math.min((ledger?.errors ?? 0) * 14, 32)
          reasons.push(`${ledger?.errors ?? 0} ledger errors in recent timeline`)
        }

        if ((ledger?.pending ?? 0) > 0) {
          score += Math.min((ledger?.pending ?? 0) * 4, 12)
          reasons.push(`${ledger?.pending ?? 0} pending placements still open`)
        }

        if (state?.latest_error) {
          score += 10
          reasons.push(state.latest_error)
        }

        const uniqueReasons = Array.from(new Set(reasons.filter(Boolean))).slice(0, 3)
        const latestAtCandidates = [ledger?.latestAt, state?.latest_transition_at, state?.latest_snapshot_at, health?.last_success].filter(Boolean) as string[]
        const latestAt = latestAtCandidates.sort((a, b) => new Date(b).getTime() - new Date(a).getTime())[0] ?? null

        return {
          key,
          name: readiness?.name ?? account?.bookmaker ?? health?.bookmaker ?? ledger?.bookmaker ?? state?.bookmaker ?? key,
          score,
          tone: score >= 55 ? 'danger' : score >= 25 ? 'warning' : score > 0 ? 'info' : 'success',
          parserStatus: health?.status ?? readiness?.healthStatus ?? null,
          readinessStage: readiness?.stage ?? 'production',
          ledgerErrors: ledger?.errors ?? 0,
          pendingPlacements: ledger?.pending ?? 0,
          accountBlocked: Boolean(account && !account.readiness.placement_ready),
          safeModeBlocked: Boolean(account?.readiness.submit_blocked_by_safe_mode),
          approvalRequired: Boolean(account?.readiness.approval_required),
          latestAt,
          reasons: uniqueReasons,
          timelineBookmaker: ledger?.bookmaker ?? account?.bookmaker ?? health?.bookmaker ?? state?.bookmaker ?? readiness?.name ?? null,
        } satisfies BookmakerHotspot
      })
      .sort((a, b) => b.score - a.score || b.ledgerErrors - a.ledgerErrors || a.name.localeCompare(b.name))
      .slice(0, 6)
  }, [accountStates, parserHealth, readinessRows, stateDiagnostics, timelineItems])
  const hotspotSummary = useMemo(() => ({
    danger: bookmakerHotspots.filter((entry) => entry.tone === 'danger').length,
    warning: bookmakerHotspots.filter((entry) => entry.tone === 'warning').length,
    active: bookmakerHotspots.filter((entry) => entry.score > 0).length,
  }), [bookmakerHotspots])
  const freebetSignals = useMemo(() => ({
    milestones: (freebetSummary?.next_milestones ?? []).slice(0, 3),
    blockers: (freebetSummary?.blockers ?? []).slice(0, 3),
    focuses: (freebetSummary?.read_only_focuses ?? []).slice(0, 3),
  }), [freebetSummary])
  const freebetOperatorSnapshot = useMemo(() => {
    const blockedAccounts = accountStates.filter((account) => !account.readiness.placement_ready).length
    const executionReady = accounts?.ready_for_execution ?? accountStates.filter((account) => account.readiness.placement_ready).length
    const parserWatchers = parserHealthSummary.incidents + parserHealthSummary.degraded
    const freebetBlocked = freebetSummary?.blocked_states ?? 0
    const fundingGap = freebetSummary?.total_funding_gap ?? 0
    const trackedPlans = freebetSummary?.tracked_plans ?? 0
    const largestGap = freebetSummary?.largest_funding_gap ?? null
    const tone = freebetBlocked > 0 || fundingGap > 0 || blockedAccounts > 0 || parserHealthSummary.incidents > 0
      ? 'warning'
      : trackedPlans > 0 && executionReady > 0
        ? 'success'
        : 'info'

    return {
      tone,
      pills: [
        { label: 'plans', value: trackedPlans, badge: 'badge-info' },
        { label: 'exec-ready', value: executionReady, badge: executionReady > 0 ? 'badge-success' : 'badge-info' },
        { label: 'parser watch', value: parserWatchers, badge: parserWatchers > 0 ? 'badge-warning' : 'badge-success' },
        { label: 'blocked states', value: freebetBlocked, badge: freebetBlocked > 0 ? 'badge-warning' : 'badge-success' },
      ],
      summary: largestGap
        ? `${largestGap.bookmaker} remains the main freebet funding gap at ${largestGap.amount.toFixed(2)} RUB.`
        : trackedPlans > 0
          ? 'Tracked freebet plans are visible without a leading funding gap in the current snapshot.'
          : 'No tracked freebet plans are visible in the current lifecycle snapshot.',
      detail: freebetBlocked > 0
        ? `${freebetBlocked} lifecycle states are blocked; compare with ${blockedAccounts} blocked execution accounts before manual follow-up.`
        : fundingGap > 0
          ? `Funding gap totals ${fundingGap.toFixed(2)} RUB while execution-ready coverage is ${executionReady}.`
          : parserWatchers > 0
            ? `${parserWatchers} parser watchers can still affect freebet verification despite a clean lifecycle rollup.`
            : 'Lifecycle, execution readiness and parser health currently align for a read-only operator pass.',
    } as const
  }, [accountStates, accounts, freebetSummary, parserHealthSummary.degraded, parserHealthSummary.incidents])
  const topOperatorAction = operatorActionQueue[0] ?? null
  const topHotspot = bookmakerHotspots[0] ?? null
  const operatorNowStrip = useMemo(() => {
    const safeModeCount = executionReadinessSummary.submit_blocked_by_safe_mode
    const approvalCount = executionReadinessSummary.approval_required
    const hotspotCount = hotspotSummary.active

    return [
      {
        key: 'next-action',
        label: 'Next action',
        value: topOperatorAction ? topOperatorAction.bookmaker : 'Queue clear',
        detail: topOperatorAction
          ? topOperatorAction.reasons[0] ?? `${topOperatorAction.mode} requires operator follow-up.`
          : 'No operator account action is currently queued.',
        badge: topOperatorAction
          ? topOperatorAction.tone === 'danger' ? 'badge-danger' : topOperatorAction.tone === 'warning' ? 'badge-warning' : 'badge-info'
          : 'badge-success',
      },
      {
        key: 'execution-guard',
        label: 'Execution guard',
        value: safeModeCount > 0 ? `${safeModeCount} safe mode` : approvalCount > 0 ? `${approvalCount} approval` : 'Clear',
        detail: safeModeCount > 0
          ? 'Submit path is still blocked by safe mode for part of the fleet.'
          : approvalCount > 0
            ? 'Manual approval is still required before placement on some accounts.'
            : 'No safe-mode or approval blockers are visible in the current snapshot.',
        badge: safeModeCount > 0 ? 'badge-danger' : approvalCount > 0 ? 'badge-warning' : 'badge-success',
      },
      {
        key: 'top-hotspot',
        label: 'Top hotspot',
        value: topHotspot ? topHotspot.name : 'None',
        detail: topHotspot
          ? topHotspot.reasons[0] ?? `${topHotspot.ledgerErrors} ledger errors / ${topHotspot.pendingPlacements} pending placements.`
          : hotspotCount > 0
            ? `${hotspotCount} hotspot signals are active.`
            : 'No bookmaker hotspot is currently above the triage threshold.',
        badge: topHotspot
          ? topHotspot.tone === 'danger' ? 'badge-danger' : topHotspot.tone === 'warning' ? 'badge-warning' : 'badge-info'
          : 'badge-success',
      },
      {
        key: 'freebet-watch',
        label: 'Freebet watch',
        value: freebetOperatorSnapshot.pills[3]?.value?.toString() ?? '0',
        detail: freebetOperatorSnapshot.detail,
        badge: freebetOperatorSnapshot.tone === 'warning' ? 'badge-warning' : freebetOperatorSnapshot.tone === 'success' ? 'badge-success' : 'badge-info',
      },
    ]
  }, [executionReadinessSummary.approval_required, executionReadinessSummary.submit_blocked_by_safe_mode, freebetOperatorSnapshot, hotspotSummary.active, topHotspot, topOperatorAction])
  const semiAutoStats = useMemo(() => ({
    awaiting: semiAutoCoupons.filter((coupon) => coupon.status === 'awaiting_operator').length,
    blocked: semiAutoCoupons.filter((coupon) => coupon.status === 'blocked').length,
    applied: semiAutoCoupons.filter((coupon) => coupon.status === 'applied_safe_mode').length,
  }), [semiAutoCoupons])

  const handleConfirmSemiAutoCoupon = async (couponId: string) => {
    setConfirmingCouponId(couponId)
    try {
      await onConfirmSemiAutoCoupon(couponId)
    } finally {
      setConfirmingCouponId(null)
    }
  }

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="space-y-6">
      <motion.div variants={item} className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div>
          <h2 className="text-2xl font-bold">Execution / Operator</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Read-only surface поверх execution GET endpoints без управляющих действий.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <span className={`badge ${stateFreshness.badge}`}>{stateFreshness.label}</span>
          <span className={`badge ${executionTone.badge}`}>{executionTone.label}</span>
          <div className="rounded-lg px-3 py-2 text-xs" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            Snapshot {formatDateTime(executionState?.generated_at ?? executionOverview?.generated_at ?? executionLedger?.generated_at ?? null)}
          </div>
          {/* Automation mode switcher (UI prototype) */}
          <div className="ml-auto flex items-center gap-2">
            <span className="text-xs" style={{ color: 'var(--text-muted)' }}>Mode:</span>
            <button
              onClick={() => { setAutoMode('auto'); toast.success('Mode: Автомат'); }}
              className={`px-3 py-2 rounded ${autoMode === 'auto' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-800'}`}
            >
              Автомат
            </button>
            <button
              onClick={() => { setAutoMode('semi'); toast.success('Mode: Полуавтомат'); }}
              className={`px-3 py-2 rounded ${autoMode === 'semi' ? 'bg-blue-600 text-white' : 'bg-gray-200 text-gray-800'}`}
            >
              Полуавтомат
            </button>
            <span className="text-xs" style={{ color: 'var(--text-muted)' }}>{autoMode === 'auto' ? 'Автомат' : 'Полуавтомат'}</span>
          </div>
        </div>
      </motion.div>

      <motion.div variants={item}>
        <CompactSignalOverlay
          title="Operator snapshot brief"
          tone={operatorBrief.tone}
          summary={operatorBrief.summary}
          pills={operatorBrief.pills}
          actions={operatorBrief.actions}
        />
      </motion.div>

      {bookmakerCatalogCards.length > 0 && (
        <motion.div variants={item} className="grid grid-cols-1 xl:grid-cols-3 gap-3">
          {bookmakerCatalogCards.map((entry) => (
            <div key={entry.key} className="rounded-xl p-4" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
              <div className="flex items-center justify-between gap-3 mb-2">
                <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{entry.label}</p>
                <span className={`badge ${entry.badge}`}>{entry.value}</span>
              </div>
              <p className="text-xs leading-5" style={{ color: 'var(--text-secondary)' }}>{entry.detail}</p>
            </div>
          ))}
        </motion.div>
      )}

      <motion.div variants={item} className="grid grid-cols-1 xl:grid-cols-4 gap-3">
        {operatorNowStrip.map((entry) => (
          <div key={entry.key} className="rounded-xl p-4" style={{ background: 'var(--bg-card)', border: `1px solid ${entry.badge === 'badge-danger' ? 'rgba(248, 81, 73, 0.28)' : entry.badge === 'badge-warning' ? 'rgba(210, 153, 34, 0.28)' : entry.badge === 'badge-success' ? 'rgba(63, 185, 80, 0.24)' : 'var(--border-color)'}` }}>
            <div className="flex items-center justify-between gap-3 mb-2">
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{entry.label}</p>
              <span className={`badge ${entry.badge}`}>{entry.value}</span>
            </div>
            <p className="text-xs leading-5" style={{ color: 'var(--text-secondary)' }}>{entry.detail}</p>
          </div>
        ))}
      </motion.div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between mb-4">
          <div>
            <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Semi-auto coupon queue</p>
            <h3 className="text-lg font-semibold mt-1">Подготовленные ставки с ручным подтверждением</h3>
            <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
              Система готовит купон и preflight, оператор нажимает подтвердить. Remote real-money submit остаётся выключен: применяется safe-mode execution path.
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <span className="badge badge-warning">awaiting {semiAutoStats.awaiting}</span>
            <span className="badge badge-danger">blocked {semiAutoStats.blocked}</span>
            <span className="badge badge-success">applied {semiAutoStats.applied}</span>
          </div>
        </div>

        {semiAutoCoupons.length === 0 ? (
          <div className="rounded-xl p-4 text-sm" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-secondary)' }}>
            Нет текущих surebet-купонов для полуавто-подтверждения. Очередь появится после обнаружения вилок scanner runtime.
          </div>
        ) : (
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
            {semiAutoCoupons.slice(0, 6).map((coupon) => {
              const isReady = coupon.status === 'awaiting_operator' && coupon.all_legs_ready
              const isConfirming = confirmingCouponId === coupon.id
              const badge = coupon.status === 'applied_safe_mode'
                ? 'badge-success'
                : coupon.status === 'blocked'
                  ? 'badge-danger'
                  : 'badge-warning'

              return (
                <div key={coupon.id} className="rounded-xl p-4" style={{ background: 'var(--bg-card)', border: `1px solid ${isReady ? 'rgba(210, 153, 34, 0.32)' : 'var(--border-color)'}` }}>
                  <div className="flex items-start justify-between gap-3 mb-3">
                    <div>
                      <p className="font-semibold">{coupon.home_team} vs {coupon.away_team}</p>
                      <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>{coupon.league || 'league unknown'} • +{coupon.profit_percent.toFixed(2)}% • {coupon.total_stake.toFixed(0)} RUB</p>
                    </div>
                    <span className={`badge ${badge}`}>{coupon.status.replace(/_/g, ' ')}</span>
                  </div>

                  <div className="space-y-2 mb-3">
                    {coupon.legs.map((leg) => (
                      <div key={`${coupon.id}-${leg.bookmaker}-${leg.selection}`} className="flex items-center justify-between gap-2 rounded-lg px-3 py-2 text-xs" style={{ background: 'var(--bg-primary)' }}>
                        <span>{leg.bookmaker} • {leg.market} / {leg.selection}</span>
                        <span>{leg.odds.toFixed(2)} × {leg.stake.toFixed(0)} RUB</span>
                        <span className={`badge ${leg.preflight.dry_run_ready ? 'badge-success' : 'badge-danger'}`}>{leg.receipt?.status ?? (leg.preflight.dry_run_ready ? 'ready' : 'blocked')}</span>
                      </div>
                    ))}
                  </div>

                  {coupon.blocking_reasons.length > 0 && (
                    <p className="text-xs mb-3" style={{ color: 'var(--text-secondary)' }}>{coupon.blocking_reasons[0]}</p>
                  )}

                  <button
                    type="button"
                    disabled={!isReady || isConfirming}
                    onClick={() => handleConfirmSemiAutoCoupon(coupon.id)}
                    className={`w-full rounded-lg px-4 py-2 text-sm font-semibold ${isReady ? 'bg-blue-600 text-white hover:bg-blue-500' : 'bg-gray-700 text-gray-300 cursor-not-allowed'}`}
                  >
                    {isConfirming ? 'Применяем...' : coupon.status === 'applied_safe_mode' ? 'Уже применено safe-mode' : 'Подтвердить и применить'}
                  </button>
                </div>
              )
            })}
          </div>
        )}
      </motion.div>

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-4">
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-start justify-between mb-4">
            <div>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Execution overview</p>
              <p className="text-xl font-semibold mt-1">{executionStatus?.running ? 'Running' : executionStatus?.enabled ? 'Standby' : 'Disabled'}</p>
            </div>
            <ToneIcon size={20} style={{ color: executionTone.accent }} />
          </div>
          <div className="space-y-2 text-sm">
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Today</span><span>{executionStatus?.bets_placed_today ?? 0} bets</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Total</span><span>{executionStatus?.bets_placed_total ?? 0} bets</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>P&L day</span><span className={executionStatus && executionStatus.profit_today >= 0 ? 'profit-positive' : 'profit-negative'}>{formatCurrency(executionStatus?.profit_today ?? 0)}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Errors today</span><span className={executionStatus && executionStatus.errors_today > 0 ? 'profit-negative' : 'profit-positive'}>{executionStatus?.errors_today ?? 0}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Last bet</span><span>{formatDateTime(executionStatus?.last_bet ?? null)}</span></div>
          </div>
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-start justify-between mb-4">
            <div>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Accounts readiness</p>
              <p className="text-xl font-semibold mt-1">{accounts?.ready_for_execution ?? 0} / {accounts?.total_bookmakers ?? readinessRows.length}</p>
            </div>
            <Wallet size={20} style={{ color: 'var(--accent-blue)' }} />
          </div>
          <div className="space-y-2 text-sm">
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Ready for execution</span><span>{formatPercent(readyRate)}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Authenticated sessions</span><span>{accounts?.sessions_authenticated ?? 0} / {accounts?.sessions_configured ?? 0}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Session auth rate</span><span>{formatPercent(authRate)}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Balances cached</span><span>{accounts?.balances_cached ?? 0}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Control issues</span><span className={(accounts?.accounts_with_control_issues ?? 0) > 0 ? 'profit-negative' : 'profit-positive'}>{accounts?.accounts_with_control_issues ?? 0}</span></div>
          </div>
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-start justify-between mb-4">
            <div>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Ledger timeline</p>
              <p className="text-xl font-semibold mt-1">{executionLedger?.total_entries ?? ledgerPlacements?.total ?? 0} records</p>
            </div>
            <Activity size={20} style={{ color: 'var(--accent-cyan)' }} />
          </div>
          <div className="space-y-2 text-sm">
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Unique placements</span><span>{executionLedger?.unique_placements ?? recentPlacements?.total ?? 0}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Pending / settled</span><span>{ledgerPlacements?.pending ?? 0} / {ledgerPlacements?.settled ?? 0}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Ledger errors</span><span className={ledgerErrorRate > 0 ? 'profit-negative' : 'profit-positive'}>{formatPercent(ledgerErrorRate)}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Latest record</span><span>{formatDateTime(executionLedger?.latest_recorded_at ?? null)}</span></div>
            <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Transitions</span><span>{stateMachine?.total_transitions ?? 0}</span></div>
          </div>
        </motion.div>
      </div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between mb-4">
          <div>
            <h3 className="text-base font-semibold">Auth / readiness execution surface</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              `/api/v1/execution/state` readiness rollup with `/api/v1/accounts` fallback.
            </p>
          </div>
          <div className="flex flex-wrap gap-2 text-xs">
            <span className="badge badge-info">auth-ready {executionReadinessSummary.auth_ready}</span>
            <span className={`badge ${executionReadinessSummary.sessions_authenticated < executionReadinessSummary.auth_ready ? 'badge-warning' : 'badge-success'}`}>live auth {executionReadinessSummary.sessions_authenticated}</span>
            <span className={`badge ${executionReadinessSummary.submit_blocked_by_safe_mode > 0 ? 'badge-danger' : 'badge-success'}`}>safe mode {executionReadinessSummary.submit_blocked_by_safe_mode}</span>
            <span className={`badge ${executionReadinessSummary.operator_attention_required > 0 ? 'badge-warning' : 'badge-success'}`}>attention {executionReadinessSummary.operator_attention_required}</span>
          </div>
        </div>

        <div className="grid grid-cols-2 xl:grid-cols-5 gap-3 mb-4">
          {[
            ['Auth ready', `${executionReadinessSummary.auth_ready} / ${executionReadinessSummary.total_bookmakers}`, formatPercent(authReadinessRate)],
            ['Authenticated', `${executionReadinessSummary.sessions_authenticated} / ${executionReadinessSummary.total_bookmakers}`, formatPercent(liveAuthRate)],
            ['Balances cached', executionReadinessSummary.balances_cached.toString(), `${executionReadinessSummary.dry_run_ready} dry-run ready`],
            ['Placement ready', `${executionReadinessSummary.placement_ready} / ${executionReadinessSummary.total_bookmakers}`, formatPercent(executionPlacementRate)],
            ['Operator attention', executionReadinessSummary.operator_attention_required.toString(), `${executionReadinessSummary.approval_required} approval / ${executionReadinessSummary.submit_blocked_by_safe_mode} safe mode`],
          ].map(([label, value, detail]) => (
            <div key={String(label)} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{label}</p>
              <p className="text-2xl font-semibold mt-2">{value}</p>
              <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>{detail}</p>
            </div>
          ))}
        </div>

        {authSurfaceRows.length > 0 ? (
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
            {authSurfaceRows.map((entry) => (
              <button
                key={entry.bookmaker}
                type="button"
                onClick={() => onOpenAccount(entry.bookmaker)}
                className="w-full rounded-xl p-4 text-left transition-colors"
                style={{
                  background: 'var(--bg-secondary)',
                  border: `1px solid ${entry.tone === 'danger' ? 'rgba(248, 81, 73, 0.22)' : entry.tone === 'warning' ? 'rgba(210, 153, 34, 0.22)' : 'var(--border-color)'}`,
                }}
              >
                <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-semibold">{entry.bookmaker}</p>
                      <span className={`badge ${entry.tone === 'danger' ? 'badge-danger' : entry.tone === 'warning' ? 'badge-warning' : 'badge-success'}`}>priority {entry.priority}</span>
                      <span className={`badge ${entry.execution_mode === 'Real' || entry.execution_mode === 'SemiRealReady' || entry.execution_mode === 'Armed' ? 'badge-success' : 'badge-info'}`}>{formatExecutionMode(entry.execution_mode)}</span>
                      {entry.requires_session ? <span className={`badge ${entry.session_authenticated ? 'badge-success' : entry.auth_ready ? 'badge-warning' : 'badge-danger'}`}>{entry.session_authenticated ? 'session active' : entry.auth_ready ? 'auth drift' : 'auth blocked'}</span> : <span className="badge badge-info">no session req</span>}
                    </div>

                    <div className="flex flex-wrap gap-2 mt-2">
                      <span className={`badge ${entry.balance_cached ? 'badge-success' : 'badge-warning'}`}>balance {entry.balance_cached ? 'cached' : 'missing'}</span>
                      <span className={`badge ${entry.dry_run_ready ? 'badge-success' : 'badge-info'}`}>dry-run {entry.dry_run_ready ? 'ready' : 'off'}</span>
                      <span className={`badge ${entry.placement_ready ? 'badge-success' : 'badge-warning'}`}>placement {entry.placement_ready ? 'ready' : 'blocked'}</span>
                      {entry.approval_required ? <span className="badge badge-warning">approval</span> : null}
                      {entry.submit_blocked_by_safe_mode ? <span className="badge badge-danger">safe mode</span> : null}
                    </div>

                    <div className="mt-3 space-y-1">
                      {entry.issues.length > 0 ? entry.issues.slice(0, 2).map((issue) => (
                        <p key={`${entry.bookmaker}-${issue}`} className="text-xs" style={{ color: 'var(--text-secondary)' }}>{issue}</p>
                      )) : <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Auth and readiness path is clean in the current snapshot.</p>}
                    </div>
                  </div>

                  <div className="text-left lg:text-right shrink-0">
                    <p className="text-xs" style={{ color: 'var(--text-muted)' }}>execution path</p>
                    <p className="text-sm font-medium">{entry.account_configured ? entry.account_enabled ? 'Configured / enabled' : 'Configured / disabled' : 'No account'}</p>
                    <p className="text-xs mt-1" style={{ color: 'var(--accent-blue)' }}>open accounts drill-down</p>
                  </div>
                </div>
              </button>
            ))}
          </div>
        ) : (
          <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
            <ShieldCheck size={24} className="mx-auto mb-3 opacity-40" />
            <p className="text-sm">Execution auth/readiness surface пока пустой: UI ждёт readiness rollup из `/api/v1/execution/state`.</p>
          </div>
        )}
      </motion.div>

      <motion.div variants={item} className="panel">
        <div className="panel-header">
          <div>
            <h3>Operator queue</h3>
            <p className="text-sm text-muted">Prioritized actions from `/api/v1/execution/operator-queue`</p>
          </div>
          <span className={executionOperatorQueue?.critical_items ? 'badge badge-danger' : executionOperatorQueue?.warning_items ? 'badge badge-warning' : 'badge badge-success'}>
            {executionOperatorQueue ? `${executionOperatorQueue.critical_items} critical / ${executionOperatorQueue.warning_items} warning` : 'No queue'}
          </span>
        </div>

        {operatorQueueRows.length > 0 ? (
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
            {operatorQueueRows.map((entry) => (
              <button
                key={`queue-${entry.bookmaker}`}
                type="button"
                onClick={() => onOpenAccount(entry.bookmaker)}
                className="w-full rounded-xl p-4 text-left transition-colors"
                style={{
                  background: 'var(--bg-secondary)',
                  border: `1px solid ${entry.severity === 'critical' ? 'rgba(248, 81, 73, 0.22)' : entry.severity === 'warning' ? 'rgba(210, 153, 34, 0.22)' : 'var(--border-color)'}`,
                }}
              >
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-semibold">{entry.bookmaker}</p>
                      <span className={`badge ${entry.severity === 'critical' ? 'badge-danger' : entry.severity === 'warning' ? 'badge-warning' : 'badge-info'}`}>{entry.severity}</span>
                      <span className="badge badge-info">score {entry.priority_score}</span>
                      <span className={`badge ${entry.placement_ready ? 'badge-success' : 'badge-warning'}`}>{entry.placement_ready ? 'placement ready' : 'placement blocked'}</span>
                    </div>
                    <p className="mt-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
                      {entry.operator_action ?? entry.blocking_reasons[0] ?? entry.latest_error ?? 'Review execution state drift'}
                    </p>
                    <div className="mt-2 flex flex-wrap gap-2 text-xs" style={{ color: 'var(--text-muted)' }}>
                      <span>mode {formatExecutionMode(entry.execution_mode as BookmakerExecutionMode | null)}</span>
                      {entry.approval_required ? <span>approval required</span> : null}
                      {entry.submit_blocked_by_safe_mode ? <span>safe mode blocks submit</span> : null}
                      {entry.session_stale ? <span>session stale</span> : null}
                      {entry.balance_stale ? <span>balance stale</span> : null}
                      {entry.auth_snapshot_stale ? <span>auth snapshot stale</span> : null}
                    </div>
                  </div>
                  <div className="text-right text-xs" style={{ color: 'var(--accent-blue)' }}>
                    open accounts drill-down
                  </div>
                </div>
              </button>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted">Operator queue is empty: no prioritized auth/execution actions detected.</p>
        )}
      </motion.div>

      {attentionCards.length > 0 ? (
        <motion.div variants={item} className="grid grid-cols-1 xl:grid-cols-4 gap-3">
          {attentionCards.map((entry) => (
            <div key={entry.key} className="rounded-xl p-4" style={{ background: 'var(--bg-card)', border: `1px solid ${entry.badge === 'badge-danger' ? 'rgba(248, 81, 73, 0.3)' : 'rgba(210, 153, 34, 0.3)'}` }}>
              <div className="flex items-center justify-between gap-3 mb-2">
                <p className="text-sm font-semibold">{entry.title}</p>
                <span className={`badge ${entry.badge}`}><Siren size={12} /> attention</span>
              </div>
              <p className="text-xs leading-5" style={{ color: 'var(--text-secondary)' }}>{entry.detail}</p>
            </div>
          ))}
        </motion.div>
      ) : null}

      <div className="grid grid-cols-1 xl:grid-cols-[1.35fr,0.85fr] gap-6">
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Accounts readiness</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                `/api/v1/bookmakers` + `/api/v1/parsers/coverage`
              </p>
            </div>
            <div className="flex flex-wrap gap-2 text-xs">
              <span className="badge badge-success">exec {readinessSummary.executionReady}</span>
              <span className="badge badge-info">prod {readinessSummary.production}</span>
              <span className="badge badge-danger">blocked {readinessSummary.blocked}</span>
              <span className="badge badge-warning">warn {readinessSummary.warnings}</span>
            </div>
          </div>

          <div className="flex flex-wrap gap-2 mb-4">
            {([
              ['attention', `attention ${readinessRows.filter((entry) => entry.hasAttention).length}`, 'badge-danger'],
              ['execution', `execution ${readinessRows.filter((entry) => entry.executionSupported).length}`, 'badge-success'],
              ['blocked', `blocked ${readinessRows.filter((entry) => entry.stage === 'blocked').length}`, 'badge-warning'],
              ['all', `all ${readinessRows.length}`, 'badge-info'],
            ] as const).map(([value, label, badge]) => (
              <button key={value} type="button" onClick={() => setReadinessFilter(value)} className={`badge ${readinessFilter === value ? badge : 'badge-info'}`} style={{ opacity: readinessFilter === value ? 1 : 0.7 }}>
                {label}
              </button>
            ))}
          </div>

          <div className="space-y-4">
            {readinessGroups.map((group) => group.rows.length > 0 ? (
              <div key={group.key} className="space-y-3">
                <div className="flex items-center justify-between gap-3">
                  <div>
                    <h4 className="text-sm font-semibold">{group.title}</h4>
                    <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>{group.description}</p>
                  </div>
                  <span className="badge badge-info">{group.rows.length}</span>
                </div>

                {group.rows.map((entry) => (
                  <div key={entry.slug} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: `1px solid ${entry.hasAttention ? 'rgba(248, 81, 73, 0.22)' : 'var(--border-color)'}` }}>
                    <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                      <div>
                        <div className="flex flex-wrap items-center gap-2">
                          <p className="text-sm font-semibold">{entry.name}</p>
                          <span className={`badge ${entry.executionSupported ? 'badge-success' : 'badge-info'}`}>{entry.mode}</span>
                          <span className={`badge ${readinessBadgeClass(entry.stage)}`}>{entry.stage.replace('_', ' ')}</span>
                          {entry.healthStatus ? <span className={`badge ${healthBadgeClass(entry.healthStatus)}`}>{entry.healthStatus}</span> : null}
                        </div>
                        <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                          {entry.scanSupported ? 'scan' : 'no scan'} • {entry.executionSupported ? 'execution contract ready' : 'execution not exposed'} • backend {entry.backendStatus}
                          {entry.uptimePercent !== null ? ` • uptime ${entry.uptimePercent.toFixed(1)}%` : ''}
                          {entry.avgResponseTimeMs !== null ? ` • avg ${entry.avgResponseTimeMs.toFixed(0)} ms` : ''}
                        </p>
                        {entry.notes ? (
                          <p className="text-xs mt-2" style={{ color: 'var(--text-muted)' }}>{entry.notes}</p>
                        ) : null}
                        {entry.lastError ? (
                          <p className="text-xs mt-2 profit-negative">{entry.lastError}</p>
                        ) : null}
                        {entry.failingChecks.slice(0, 2).map((check) => (
                          <p key={`${entry.slug}-${check.code}-message`} className="text-xs mt-2" style={{ color: check.severity === 'fail' ? 'var(--accent-red)' : 'var(--accent-yellow)' }}>
                            {check.code}: {check.message}
                          </p>
                        ))}
                      </div>

                      <div className="flex flex-wrap gap-2 lg:max-w-[40%] lg:justify-end">
                        {entry.checks.slice(0, 4).map((check) => (
                          <span key={`${entry.slug}-${check.code}`} className={`badge ${check.severity === 'fail' ? 'badge-danger' : check.severity === 'warn' ? 'badge-warning' : check.severity === 'pass' ? 'badge-success' : 'badge-info'}`}>
                            {check.code}
                          </span>
                        ))}
                        {entry.consecutiveFailures > 0 ? <span className="badge badge-danger">fails {entry.consecutiveFailures}</span> : null}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            ) : null)}

            {filteredReadinessRows.length === 0 ? (
              <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
                <ShieldCheck size={24} className="mx-auto mb-3 opacity-40" />
                <p className="text-sm">Текущий фильтр не нашёл account issues в read-only snapshots.</p>
              </div>
            ) : null}
          </div>
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">State machine</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                `/api/v1/execution/state` with overview / ledger fallback
              </p>
            </div>
            <TimerReset size={16} style={{ color: 'var(--text-muted)' }} />
          </div>

          <div className="grid grid-cols-2 gap-3 mb-4">
            {[
              { label: 'Pending', value: stateMachineSnapshot?.phases.pending_placement ?? 0, icon: Clock3, color: 'var(--accent-yellow)' },
              { label: 'Confirmed', value: stateMachineSnapshot?.phases.confirmed_placement ?? 0, icon: CheckCircle2, color: 'var(--accent-green)' },
              { label: 'Settled', value: stateMachineSnapshot?.phases.settled ?? 0, icon: ShieldCheck, color: 'var(--accent-blue)' },
              { label: 'Failed', value: stateMachineSnapshot?.phases.failed ?? 0, icon: ShieldAlert, color: 'var(--accent-red)' },
            ].map((entry) => {
              const Icon = entry.icon

              return (
                <div key={entry.label} className="rounded-lg p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{entry.label}</span>
                    <Icon size={14} style={{ color: entry.color }} />
                  </div>
                  <p className="text-2xl font-semibold">{entry.value}</p>
                </div>
              )
            })}
          </div>

          <div className="space-y-3">
            <div className="rounded-lg p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider mb-1" style={{ color: 'var(--text-muted)' }}>Freshness</p>
              <div className="flex flex-wrap items-center gap-2">
                <span className={`badge ${stateFreshness.badge}`}>{stateFreshness.label}</span>
                <span className="text-sm">{stateFreshness.detail}</span>
              </div>
            </div>
            <div className="rounded-lg p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider mb-1" style={{ color: 'var(--text-muted)' }}>Recent placements</p>
              <p className="text-sm">{recentPlacements?.placed ?? 0} placed / {recentPlacements?.pending ?? 0} pending / {recentPlacements?.errors ?? 0} errors</p>
            </div>
            <div className="rounded-lg p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider mb-1" style={{ color: 'var(--text-muted)' }}>Latest transition</p>
              <p className="text-sm">{formatDateTime(stateMachineSnapshot?.latest_transition_at ?? null)}</p>
            </div>
            <div className="rounded-lg p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider mb-1" style={{ color: 'var(--text-muted)' }}>Latest snapshot</p>
              <p className="text-sm">{formatDateTime(stateMachineSnapshot?.latest_snapshot_at ?? null)}</p>
            </div>
          </div>
        </motion.div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[0.95fr,1.05fr] gap-6">
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Execution state diagnostics</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                `/api/v1/execution/state` as troubleshooting surface
              </p>
            </div>
            {stateDiagnostics ? <span className="badge badge-info">{stateDiagnostics.total_transitions} transitions</span> : null}
          </div>

          {bookmakerStateRows.length > 0 ? (
            <div className="space-y-3">
              {bookmakerStateRows.map((entry) => (
                <div key={entry.bookmaker} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: `1px solid ${entry.latest_error ? 'rgba(248, 81, 73, 0.22)' : 'var(--border-color)'}` }}>
                  <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                    <div>
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-semibold">{entry.bookmaker}</p>
                        <span className={`badge ${entry.latest_error ? 'badge-danger' : 'badge-success'}`}>{entry.total_snapshots} snapshots</span>
                      </div>
                      <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                        Pending {entry.phases.pending_placement} • Confirmed {entry.phases.confirmed_placement} • Settled {entry.phases.settled} • Failed {entry.phases.failed}
                      </p>
                      {entry.latest_error ? <p className="text-xs mt-2 profit-negative">{entry.latest_error}</p> : null}
                    </div>

                    <div className="text-left lg:text-right shrink-0">
                      <p className="text-xs" style={{ color: 'var(--text-muted)' }}>snapshot {formatDateTime(entry.latest_snapshot_at)}</p>
                      <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>transition {formatDateTime(entry.latest_transition_at)}</p>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
              <Activity size={24} className="mx-auto mb-3 opacity-40" />
              <p className="text-sm">Execution state endpoint пока не отдал bookmaker diagnostics.</p>
            </div>
          )}
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Recent state transitions</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                Error-first replay from `/api/v1/execution/state`
              </p>
            </div>
            {transitionRows.length > 0 ? <span className="badge badge-info">{transitionRows.length} shown</span> : null}
          </div>

          {transitionRows.length > 0 ? (
            <div className="space-y-3">
              {transitionRows.map((entry) => (
                <div key={`${entry.placement_id}-${entry.sequence}`} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: `1px solid ${entry.error ? 'rgba(248, 81, 73, 0.22)' : 'var(--border-color)'}` }}>
                  <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                    <div>
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-semibold">{entry.bookmaker}</p>
                        <span className={`badge ${entry.error ? 'badge-danger' : entry.to_phase.toLowerCase().includes('failed') ? 'badge-danger' : entry.to_phase.toLowerCase().includes('settled') ? 'badge-success' : 'badge-warning'}`}>{toTitleCase(entry.to_phase)}</span>
                        <span className="badge badge-info">seq {entry.sequence}</span>
                      </div>
                      <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                        {entry.from_phase ? `${toTitleCase(entry.from_phase)} -> ${toTitleCase(entry.to_phase)}` : `Entered ${toTitleCase(entry.to_phase)}`} • {toTitleCase(entry.placement_status)} • {toTitleCase(entry.action)}
                      </p>
                      {entry.error ? <p className="text-xs mt-2 profit-negative">{entry.error}</p> : null}
                    </div>

                    <div className="text-left lg:text-right shrink-0">
                      <p className="text-sm font-medium">{formatDateTime(entry.occurred_at)}</p>
                      <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.placement_id}</p>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
              <AlertTriangle size={24} className="mx-auto mb-3 opacity-40" />
              <p className="text-sm">Execution state replay пока пустой: UI ждёт transitions из `/api/v1/execution/state`.</p>
            </div>
          )}
        </motion.div>
      </div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex items-center justify-between mb-4 gap-4">
          <div>
            <h3 className="text-base font-semibold">Operator action queue</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              Account-level review hints from `/api/v1/accounts` without execution mutations.
            </p>
          </div>
          {operatorActionQueue.length > 0 ? <span className="badge badge-info">{operatorActionQueue.length} queued</span> : null}
        </div>

        {operatorActionQueue.length > 0 ? (
          <div className="space-y-3">
            <div className="grid grid-cols-2 xl:grid-cols-4 gap-3">
              {[
                ['Safe-mode blocks', operatorActionSummary.safeMode, 'badge-danger'],
                ['Approval required', operatorActionSummary.approval, 'badge-warning'],
                ['Session restore', operatorActionSummary.sessionRestore, 'badge-warning'],
                ['Balance refresh', operatorActionSummary.balanceRefresh, 'badge-info'],
              ].map(([label, value, badge]) => (
                <div key={String(label)} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{label}</p>
                    <span className={`badge ${badge}`}>{value}</span>
                  </div>
                </div>
              ))}
            </div>

            <div className="space-y-3">
              {operatorActionQueue.map((entry, index) => (
                <button
                  key={`${entry.bookmaker}-${index}`}
                  type="button"
                  onClick={() => onOpenAccount(entry.bookmaker)}
                  className="w-full rounded-xl p-4 text-left transition-colors"
                  style={{ background: 'var(--bg-secondary)', border: `1px solid ${entry.tone === 'danger' ? 'rgba(248, 81, 73, 0.22)' : entry.tone === 'warning' ? 'rgba(210, 153, 34, 0.22)' : 'var(--border-color)'}` }}
                >
                  <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                    <div>
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="badge badge-info">#{index + 1}</span>
                        <p className="text-sm font-semibold">{entry.bookmaker}</p>
                        <span className={`badge ${entry.tone === 'danger' ? 'badge-danger' : entry.tone === 'warning' ? 'badge-warning' : 'badge-info'}`}>{entry.mode}</span>
                        <span className={`badge ${entry.sessionState.toLowerCase().includes('auth') || entry.sessionState.toLowerCase().includes('ready') ? 'badge-success' : 'badge-warning'}`}>{entry.sessionState}</span>
                      </div>
                      <div className="mt-2 space-y-1">
                        {entry.reasons.slice(0, 3).map((reason) => (
                          <p key={`${entry.bookmaker}-${reason}`} className="text-xs" style={{ color: 'var(--text-secondary)' }}>{reason}</p>
                        ))}
                      </div>
                      <p className="text-xs mt-3" style={{ color: 'var(--accent-blue)' }}>
                        Open account drill-down
                      </p>
                    </div>

                    <div className="text-left lg:text-right shrink-0">
                      <p className="text-xs" style={{ color: 'var(--text-muted)' }}>available balance</p>
                      <p className="text-sm font-medium">{entry.availableBalance !== null ? `${entry.availableBalance.toFixed(2)} RUB` : 'no balance snapshot'}</p>
                    </div>
                  </div>
                </button>
              ))}
            </div>
          </div>
        ) : (
          <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
            <ShieldCheck size={24} className="mx-auto mb-3 opacity-40" />
            <p className="text-sm">Account snapshot не показал operator actions: execution path выглядит чистым в текущем GET-срезе.</p>
          </div>
        )}
      </motion.div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex items-center justify-between mb-4 gap-4">
          <div>
            <h3 className="text-base font-semibold">Freebet lifecycle</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              Read-only rollup from `/api/v1/freebets/summary` for operator/freebet triage.
            </p>
          </div>
          {freebetSummary ? <span className="badge badge-info">{freebetSummary.total_bookmakers} tracked</span> : null}
        </div>

        {freebetSummary && freebetSummary.total_bookmakers > 0 ? (
          <div className="space-y-4">
            <div className="grid grid-cols-2 xl:grid-cols-4 gap-3">
              {[
                ['Blocked states', freebetSummary.blocked_states, freebetSummary.blocked_states > 0 ? 'badge-warning' : 'badge-success'],
                ['Funding gaps', `${freebetSummary.total_funding_gap.toFixed(2)} RUB`, freebetSummary.total_funding_gap > 0 ? 'badge-danger' : 'badge-success'],
                ['Tracked plans', freebetSummary.tracked_plans, 'badge-info'],
                ['Est. profit', `${freebetSummary.total_estimated_profit.toFixed(2)} RUB`, 'badge-success'],
              ].map(([label, value, badge]) => (
                <div key={String(label)} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{label}</p>
                    <span className={`badge ${badge}`}>{value}</span>
                  </div>
                </div>
              ))}
            </div>

            <div
              className="rounded-xl p-4"
              style={{
                background: 'var(--bg-secondary)',
                border: `1px solid ${freebetOperatorSnapshot.tone === 'warning' ? 'rgba(210, 153, 34, 0.28)' : freebetOperatorSnapshot.tone === 'success' ? 'rgba(63, 185, 80, 0.24)' : 'var(--border-color)'}`,
              }}
            >
              <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                <div>
                  <div className="flex flex-wrap items-center gap-2 mb-2">
                    <p className="text-sm font-semibold">Operator crossover</p>
                    <span className={`badge ${freebetOperatorSnapshot.tone === 'warning' ? 'badge-warning' : freebetOperatorSnapshot.tone === 'success' ? 'badge-success' : 'badge-info'}`}>
                      read-only linked view
                    </span>
                  </div>
                  <p className="text-sm">{freebetOperatorSnapshot.summary}</p>
                  <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>{freebetOperatorSnapshot.detail}</p>
                </div>

                <div className="flex flex-wrap gap-2 lg:max-w-[42%] lg:justify-end">
                  {freebetOperatorSnapshot.pills.map((pill) => (
                    <span key={pill.label} className={`badge ${pill.badge}`}>
                      {pill.label} {pill.value}
                    </span>
                  ))}
                </div>
              </div>
            </div>

            <div className="grid grid-cols-1 xl:grid-cols-[0.9fr,1.1fr] gap-4">
              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex flex-wrap gap-2 mb-3 text-xs">
                  <span className="badge badge-info">discovered {freebetSummary.discovered}</span>
                  <span className="badge badge-info">available {freebetSummary.available}</span>
                  <span className="badge badge-warning">qualified {freebetSummary.qualified}</span>
                  <span className="badge badge-warning">planned {freebetSummary.planned}</span>
                  <span className="badge badge-info">rollover {freebetSummary.rollover_in_progress}</span>
                  <span className="badge badge-success">completed {freebetSummary.rollover_completed}</span>
                </div>
                <p className="text-sm">
                  {freebetSummary.largest_funding_gap
                    ? `Largest funding gap: ${freebetSummary.largest_funding_gap.bookmaker} needs ${freebetSummary.largest_funding_gap.amount.toFixed(2)} RUB.`
                    : 'No funding leader is currently surfaced by the lifecycle snapshot.'}
                </p>
                <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>
                  Snapshot {formatDateTime(freebetSummary.generated_at)} • amount {freebetSummary.total_freebet_amount.toFixed(2)} RUB
                </p>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                {[
                  { label: 'Next milestones', rows: freebetSignals.milestones, badge: 'badge-info' },
                  { label: 'Top blockers', rows: freebetSignals.blockers, badge: 'badge-warning' },
                  { label: 'Read-only focus', rows: freebetSignals.focuses, badge: 'badge-success' },
                ].map((section) => (
                  <div key={section.label} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                    <p className="text-xs uppercase tracking-wider mb-3" style={{ color: 'var(--text-muted)' }}>{section.label}</p>
                    {section.rows.length > 0 ? (
                      <div className="space-y-2">
                        {section.rows.map((entry) => (
                          <div key={`${section.label}-${entry.label}`} className="flex items-center justify-between gap-3 text-sm">
                            <span style={{ color: 'var(--text-secondary)' }}>{toTitleCase(entry.label)}</span>
                            <span className={`badge ${section.badge}`}>{entry.count}</span>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <p className="text-sm" style={{ color: 'var(--text-muted)' }}>No signals.</p>
                    )}
                  </div>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
            <ShieldCheck size={24} className="mx-auto mb-3 opacity-40" />
            <p className="text-sm">Freebet lifecycle snapshot пока пустой: operator surface ждёт `/api/v1/freebets/summary`.</p>
          </div>
        )}
      </motion.div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between mb-4">
          <div>
            <h3 className="text-base font-semibold">Bookmaker hotspot board</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              Unified operator ranking across parser health, account readiness and recent execution timeline.
            </p>
          </div>
          <div className="flex flex-wrap gap-2 text-xs">
            <span className={`badge ${hotspotSummary.danger > 0 ? 'badge-danger' : 'badge-success'}`}>critical {hotspotSummary.danger}</span>
            <span className={`badge ${hotspotSummary.warning > 0 ? 'badge-warning' : 'badge-success'}`}>watch {hotspotSummary.warning}</span>
            <span className="badge badge-info">active {hotspotSummary.active}</span>
          </div>
        </div>

        <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
          {bookmakerHotspots.map((entry) => (
            <button
              key={entry.key}
              type="button"
              onClick={() => {
                setLedgerBookmaker(entry.timelineBookmaker ?? 'all')
                setLedgerQuery('')
                setLedgerFilter(entry.ledgerErrors > 0 ? 'errors' : entry.pendingPlacements > 0 ? 'pending' : 'all')
              }}
              className="w-full rounded-xl p-4 text-left transition-colors"
              style={{
                background: 'var(--bg-secondary)',
                border: `1px solid ${entry.tone === 'danger' ? 'rgba(248, 81, 73, 0.22)' : entry.tone === 'warning' ? 'rgba(210, 153, 34, 0.22)' : 'var(--border-color)'}`,
              }}
            >
              <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="text-sm font-semibold">{entry.name}</p>
                    <span className={`badge ${entry.tone === 'danger' ? 'badge-danger' : entry.tone === 'warning' ? 'badge-warning' : entry.tone === 'info' ? 'badge-info' : 'badge-success'}`}>score {entry.score}</span>
                    <span className={`badge ${healthBadgeClass(entry.parserStatus)}`}>{entry.parserStatus ?? 'No runtime'}</span>
                    <span className={`badge ${readinessBadgeClass(entry.readinessStage)}`}>{entry.readinessStage.replace('_', ' ')}</span>
                  </div>

                  <div className="flex flex-wrap gap-2 mt-2">
                    {entry.accountBlocked ? <span className="badge badge-warning">account blocked</span> : <span className="badge badge-success">account ready</span>}
                    {entry.safeModeBlocked ? <span className="badge badge-danger">safe mode</span> : null}
                    {entry.approvalRequired ? <span className="badge badge-warning">approval</span> : null}
                    {entry.ledgerErrors > 0 ? <span className="badge badge-danger">ledger {entry.ledgerErrors} errors</span> : null}
                    {entry.pendingPlacements > 0 ? <span className="badge badge-info">pending {entry.pendingPlacements}</span> : null}
                  </div>

                  <div className="mt-3 space-y-1">
                    {entry.reasons.map((reason) => (
                      <p key={`${entry.key}-${reason}`} className="text-xs" style={{ color: 'var(--text-secondary)' }}>{reason}</p>
                    ))}
                    {entry.reasons.length === 0 ? <p className="text-xs" style={{ color: 'var(--text-muted)' }}>No active blockers surfaced in the current read-only snapshot.</p> : null}
                  </div>
                </div>

                <div className="text-left lg:text-right shrink-0">
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>latest signal</p>
                  <p className="text-sm font-medium">{formatDateTime(entry.latestAt)}</p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>click to focus ledger</p>
                </div>
              </div>
            </button>
          ))}
        </div>
      </motion.div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex items-center justify-between mb-4 gap-4">
          <div>
            <h3 className="text-base font-semibold">Ledger timeline</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              Последние execution records в read-only режиме.
            </p>
          </div>
          {timelineItems.length > 0 ? <span className="badge badge-info">{filteredTimelineItems.length} / {timelineItems.length} items</span> : null}
        </div>

        {timelineItems.length > 0 ? (
          <div className="space-y-3">
            <div className="grid grid-cols-1 xl:grid-cols-4 gap-3 mb-2">
              {[
                { key: 'errors', label: 'Errors first', value: ledgerSummary.errors, badge: 'badge-danger' },
                { key: 'pending', label: 'Pending', value: ledgerSummary.pending, badge: 'badge-warning' },
                { key: 'active', label: 'Open lane', value: ledgerSummary.active, badge: 'badge-info' },
                { key: 'settled', label: 'Settled', value: ledgerSummary.settled, badge: 'badge-success' },
              ].map((entry) => (
                <button
                  key={entry.key}
                  type="button"
                  onClick={() => setLedgerFilter(entry.key as LedgerFilter)}
                  className="rounded-xl p-4 text-left transition-colors"
                  style={{
                    background: 'var(--bg-secondary)',
                    border: `1px solid ${ledgerFilter === entry.key ? 'var(--accent-blue)' : 'var(--border-color)'}`,
                  }}
                >
                  <div className="flex items-center justify-between gap-3 mb-2">
                    <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{entry.label}</p>
                    <span className={`badge ${entry.badge}`}>{entry.value}</span>
                  </div>
                  <p className="text-lg font-semibold">{entry.value}</p>
                </button>
              ))}
            </div>

            <div className="grid grid-cols-1 xl:grid-cols-[1.4fr,0.8fr,0.8fr] gap-3">
              <label className="rounded-xl px-3 py-2 flex items-center gap-2" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <Search size={14} style={{ color: 'var(--text-muted)' }} />
                <input
                  value={ledgerQuery}
                  onChange={(event) => setLedgerQuery(event.target.value)}
                  placeholder="Search bookmaker, event, market, error"
                  className="w-full bg-transparent text-sm outline-none"
                  style={{ color: 'var(--text-primary)' }}
                />
              </label>

              <select
                value={ledgerBookmaker}
                onChange={(event) => setLedgerBookmaker(event.target.value)}
                className="rounded-xl px-3 py-2 text-sm"
                style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
              >
                <option value="all">All bookmakers</option>
                {ledgerBookmakerOptions.map((option) => (
                  <option key={option} value={option}>{option}</option>
                ))}
              </select>

              <select
                value={ledgerSort}
                onChange={(event) => setLedgerSort(event.target.value as LedgerSort)}
                className="rounded-xl px-3 py-2 text-sm"
                style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
              >
                <option value="priority">Sort: triage priority</option>
                <option value="newest">Sort: newest first</option>
                <option value="oldest">Sort: oldest first</option>
                <option value="stake">Sort: highest stake</option>
                <option value="odds">Sort: highest odds</option>
              </select>
            </div>

            {triageQueue.length > 0 ? (
              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex items-center justify-between gap-3 mb-3">
                  <div>
                    <h4 className="text-sm font-semibold">Operator triage</h4>
                    <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                      Error-first queue from current GET snapshot.
                    </p>
                  </div>
                  <span className="badge badge-danger">top {triageQueue.length}</span>
                </div>

                <div className="space-y-2">
                  {triageQueue.map((entry, index) => (
                    <div key={`triage-${entry.id}`} className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid var(--border-color)' }}>
                      <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                        <div>
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="badge badge-info">#{index + 1}</span>
                            <p className="text-sm font-semibold">{entry.title}</p>
                            <span className={`badge ${entry.error ? 'badge-danger' : entry.status.toLowerCase().includes('pending') ? 'badge-warning' : 'badge-success'}`}>
                              {toTitleCase(entry.status)}
                            </span>
                          </div>
                          <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.subtitle}</p>
                          <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>{entry.meta}</p>
                          {entry.error ? <p className="text-xs mt-2 profit-negative">{entry.error}</p> : null}
                        </div>

                        <div className="text-left lg:text-right shrink-0">
                          <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{formatDateTime(entry.recordedAt)}</p>
                          <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.source === 'ledger' ? 'ledger record' : 'state snapshot'}</p>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ) : null}

            {filteredTimelineItems.map((entry) => (
              <div key={entry.id} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-semibold">{entry.title}</p>
                      <span className="badge badge-info">{entry.source}</span>
                      <span className={`badge ${entry.error ? 'badge-danger' : entry.status.toLowerCase().includes('pending') ? 'badge-warning' : 'badge-success'}`}>
                        {toTitleCase(entry.status)}
                      </span>
                    </div>
                    <p className="text-sm mt-1">{entry.subtitle}</p>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.meta}</p>
                    {entry.error ? (
                      <p className="text-xs mt-2 profit-negative">{entry.error}</p>
                    ) : null}
                  </div>

                  <div className="text-left lg:text-right shrink-0">
                    <p className="text-sm font-medium">{formatDateTime(entry.recordedAt)}</p>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                      {entry.stake !== null && entry.odds !== null ? `${entry.stake.toFixed(2)} RUB @ ${entry.odds.toFixed(2)}` : 'state snapshot'}
                    </p>
                  </div>
                </div>
              </div>
            ))}

            {filteredTimelineItems.length === 0 ? (
              <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
                <AlertTriangle size={24} className="mx-auto mb-3 opacity-40" />
                <p className="text-sm">Текущие ledger filters ничего не нашли, но snapshot сохранён и готов к следующему triage pass.</p>
              </div>
            ) : null}
          </div>
        ) : (
          <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
            <AlertTriangle size={24} className="mx-auto mb-3 opacity-40" />
            <p className="text-sm">Ledger timeline пока пустой: UI готов к live snapshot с `/api/v1/execution/ledger`.</p>
          </div>
        )}
      </motion.div>
    </motion.div>
  )
}
