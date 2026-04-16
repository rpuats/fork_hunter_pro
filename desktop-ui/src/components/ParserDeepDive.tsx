import { useMemo, useState } from 'react'
import { Activity, AlertTriangle, CheckCircle2, Gauge, Radar, ShieldAlert, ShieldCheck, ShieldX, Siren, TimerReset, Wrench, Eye } from 'lucide-react'
import { CompactSignalOverlay } from './CompactSignalOverlay'
import type { BackendDiagnosticSeverity, BackendParserHealthStatus, BackendParserReadinessStage, ParserCoverage, ParserDiagnosticCheck, ParserHealth, ParserReadiness } from '../types'

interface ParserDeepDiveProps {
  parserCoverage: ParserCoverage[]
  parserHealth: ParserHealth[]
}

type DeepDiveFilter = 'all' | 'attention' | 'runtime' | 'readiness'
type SeverityFilter = 'all' | BackendDiagnosticSeverity
type RuntimeFilter = 'all' | BackendParserHealthStatus | 'unknown'
type ReadinessFilter = 'all' | BackendParserReadinessStage
type TriageLane = 'critical' | 'stabilize' | 'rollout' | 'observe'

type MergedParserRow = {
  key: string
  name: string
  slug: string
  parserType: string | null
  source: string | null
  runtimeStatus: BackendParserHealthStatus | null
  readinessStage: BackendParserReadinessStage
  enabled: boolean
  scanSupported: boolean
  executionSupported: boolean
  productionEnabled: boolean
  avgResponseTimeMs: number | null
  uptimePercent: number | null
  consecutiveFailures: number
  eventsParsed: number
  lastSuccess: string | null
  lastError: string | null
  diagnostics: ParserDiagnosticCheck[]
  readinessChecks: ParserDiagnosticCheck[]
  rootCause: string
  operatorAction: string
  triageLane: TriageLane
  attentionScore: number
  runtimeIssue: boolean
  readinessIssue: boolean
}

function normalizeKey(value: string) {
  return value.trim().toLowerCase().replace(/\s+/g, '-')
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

function formatDurationFromNow(value: string | null) {
  if (!value) return 'no signal'

  const deltaMs = Date.now() - new Date(value).getTime()
  if (Number.isNaN(deltaMs)) return 'no signal'

  const deltaMinutes = Math.max(Math.floor(deltaMs / 60000), 0)
  if (deltaMinutes < 1) return '<1m ago'
  if (deltaMinutes < 60) return `${deltaMinutes}m ago`

  const deltaHours = Math.floor(deltaMinutes / 60)
  if (deltaHours < 24) return `${deltaHours}h ago`

  const deltaDays = Math.floor(deltaHours / 24)
  return `${deltaDays}d ago`
}

function formatReadinessStage(stage: BackendParserReadinessStage) {
  switch (stage) {
    case 'production': return 'production'
    case 'rollout_ready': return 'rollout ready'
    case 'diagnostic_only': return 'diagnostic only'
    case 'blocked': return 'blocked'
  }
}

function formatHealthStatus(status: BackendParserHealthStatus | null) {
  switch (status) {
    case 'Healthy': return 'healthy'
    case 'Degraded': return 'degraded'
    case 'Unhealthy': return 'unhealthy'
    case 'CircuitOpen': return 'circuit open'
    default: return 'unknown'
  }
}

function badgeClassBySeverity(severity: BackendDiagnosticSeverity) {
  switch (severity) {
    case 'pass': return 'badge-success'
    case 'warn': return 'badge-warning'
    case 'fail': return 'badge-danger'
    case 'info': return 'badge-info'
  }
}

function healthBadgeClass(status: BackendParserHealthStatus | null) {
  switch (status) {
    case 'Healthy': return 'badge-success'
    case 'Degraded': return 'badge-warning'
    case 'Unhealthy':
    case 'CircuitOpen':
      return 'badge-danger'
    default:
      return 'badge-info'
  }
}

function readinessBadgeClass(stage: BackendParserReadinessStage) {
  switch (stage) {
    case 'production': return 'badge-success'
    case 'rollout_ready': return 'badge-info'
    case 'diagnostic_only': return 'badge-warning'
    case 'blocked': return 'badge-danger'
  }
}

function dedupeChecks(...groups: Array<ParserDiagnosticCheck[] | undefined>) {
  const seen = new Set<string>()
  const result: ParserDiagnosticCheck[] = []

  groups.flat().forEach((check) => {
    if (!check) return
    const key = `${check.code}:${check.severity}:${check.message}`
    if (seen.has(key)) return
    seen.add(key)
    result.push(check)
  })

  return result
}

function deriveRootCause(lastError: string | null, diagnostics: ParserDiagnosticCheck[], notes: string | null) {
  if (lastError) return lastError

  const failingCheck = diagnostics.find((check) => check.severity === 'fail')
    ?? diagnostics.find((check) => check.severity === 'warn')
    ?? diagnostics.find((check) => check.severity === 'info')

  if (failingCheck) return failingCheck.message
  if (notes) return notes
  return 'No blocking signal surfaced by backend snapshot.'
}

function getReadinessStage(readiness: ParserReadiness | null, enabled: boolean) {
  return readiness?.stage ?? (enabled ? 'production' : 'blocked')
}

function scoreHealth(status: BackendParserHealthStatus | null) {
  switch (status) {
    case 'Unhealthy': return 40
    case 'CircuitOpen': return 34
    case 'Degraded': return 18
    case 'Healthy': return 0
    default: return 10
  }
}

function scoreStage(stage: BackendParserReadinessStage) {
  switch (stage) {
    case 'blocked': return 30
    case 'diagnostic_only': return 18
    case 'rollout_ready': return 8
    case 'production': return 0
  }
}

function average(values: number[]) {
  if (values.length === 0) return 0
  return values.reduce((sum, value) => sum + value, 0) / values.length
}

function getOperatorAction(entry: Pick<MergedParserRow, 'runtimeStatus' | 'readinessStage' | 'consecutiveFailures' | 'lastError' | 'diagnostics' | 'productionEnabled'>) {
  if (entry.runtimeStatus === 'Unhealthy' || entry.runtimeStatus === 'CircuitOpen' || entry.consecutiveFailures >= 3) {
    return 'Recover runtime path: inspect failures, breaker state and last parser error.'
  }

  if (entry.readinessStage === 'blocked') {
    return 'Remove readiness blocker before the parser can rejoin production.'
  }

  if (entry.readinessStage === 'diagnostic_only') {
    return 'Close diagnostic-only gaps and promote after self-check passes.'
  }

  if (entry.runtimeStatus === 'Degraded' || entry.diagnostics.some((check) => check.severity === 'warn')) {
    return 'Stabilize latency and warning signals before widening rollout.'
  }

  if (entry.readinessStage === 'rollout_ready' && !entry.productionEnabled) {
    return 'Parser is rollout-ready; validate guardrails and schedule production enablement.'
  }

  return 'Keep under watch; no operator intervention is surfaced by the snapshot.'
}

function getTriageLane(entry: Pick<MergedParserRow, 'runtimeStatus' | 'readinessStage' | 'consecutiveFailures' | 'diagnostics' | 'productionEnabled'>): TriageLane {
  const hasFail = entry.diagnostics.some((check) => check.severity === 'fail')
  const hasWarn = entry.diagnostics.some((check) => check.severity === 'warn')

  if (entry.runtimeStatus === 'Unhealthy' || entry.runtimeStatus === 'CircuitOpen' || entry.consecutiveFailures >= 3 || hasFail) {
    return 'critical'
  }

  if (entry.runtimeStatus === 'Degraded' || entry.readinessStage === 'blocked' || entry.readinessStage === 'diagnostic_only' || hasWarn) {
    return 'stabilize'
  }

  if (entry.readinessStage === 'rollout_ready' || !entry.productionEnabled) {
    return 'rollout'
  }

  return 'observe'
}

function triageBadgeClass(lane: TriageLane) {
  switch (lane) {
    case 'critical': return 'badge-danger'
    case 'stabilize': return 'badge-warning'
    case 'rollout': return 'badge-info'
    case 'observe': return 'badge-success'
  }
}

function formatTriageLane(lane: TriageLane) {
  switch (lane) {
    case 'critical': return 'critical'
    case 'stabilize': return 'stabilize'
    case 'rollout': return 'rollout'
    case 'observe': return 'observe'
  }
}

function matchesTopFilter(entry: MergedParserRow, filter: DeepDiveFilter) {
  switch (filter) {
    case 'attention':
      return entry.runtimeIssue || entry.readinessIssue
    case 'runtime':
      return entry.runtimeIssue
    case 'readiness':
      return entry.readinessIssue
    default:
      return true
  }
}

export function ParserDeepDive({ parserCoverage, parserHealth }: ParserDeepDiveProps) {
  const [filter, setFilter] = useState<DeepDiveFilter>('attention')
  const [severityFilter, setSeverityFilter] = useState<SeverityFilter>('all')
  const [codeFilter, setCodeFilter] = useState('all')
  const [runtimeFilter, setRuntimeFilter] = useState<RuntimeFilter>('all')
  const [readinessFilter, setReadinessFilter] = useState<ReadinessFilter>('all')

  const rows = useMemo(() => {
    const coverageBySlug = new Map<string, ParserCoverage>()
    const coverageByName = new Map<string, ParserCoverage>()
    const healthByBookmaker = new Map<string, ParserHealth>()

    parserCoverage.forEach((entry) => {
      coverageBySlug.set(normalizeKey(entry.slug), entry)
      coverageByName.set(normalizeKey(entry.name), entry)
    })

    parserHealth.forEach((entry) => {
      healthByBookmaker.set(normalizeKey(entry.bookmaker), entry)
    })

    const allKeys = new Set([
      ...parserCoverage.map((entry) => normalizeKey(entry.slug)),
      ...parserHealth.map((entry) => normalizeKey(entry.bookmaker)),
    ])

    return [...allKeys]
      .map((key) => {
        const health = healthByBookmaker.get(key)
        const coverage = coverageBySlug.get(key)
          ?? (health ? coverageByName.get(normalizeKey(health.bookmaker)) : null)
        if (!coverage && !health) return null

        const enabled = coverage?.enabled ?? true
        const readiness = coverage?.readiness ?? health?.readiness ?? null
        const readinessStage = getReadinessStage(readiness, enabled)
        const readinessChecks = readiness?.checks ?? []
        const diagnostics = dedupeChecks(health?.diagnostics, readinessChecks)
        const runtimeStatus = health?.status ?? null
        const runtimeIssue = runtimeStatus === 'Unhealthy' || runtimeStatus === 'CircuitOpen' || runtimeStatus === 'Degraded' || Boolean(health?.last_error)
        const readinessIssue = readinessStage === 'blocked' || readinessStage === 'diagnostic_only' || diagnostics.some((check) => check.severity === 'warn' || check.severity === 'fail')
        const attentionScore = scoreHealth(runtimeStatus)
          + scoreStage(readinessStage)
          + Math.min((health?.consecutive_failures ?? 0) * 4, 20)
          + diagnostics.filter((check) => check.severity === 'fail').length * 10
          + diagnostics.filter((check) => check.severity === 'warn').length * 4
        const triageLane = getTriageLane({
          runtimeStatus,
          readinessStage,
          consecutiveFailures: health?.consecutive_failures ?? 0,
          diagnostics,
          productionEnabled: readiness?.production_enabled ?? false,
        })

        return {
          key,
          name: coverage?.name ?? health?.bookmaker ?? key,
          slug: coverage?.slug ?? normalizeKey(health?.bookmaker ?? key),
          parserType: coverage?.parser_type ?? null,
          source: coverage?.source ?? null,
          runtimeStatus,
          readinessStage,
          enabled,
          scanSupported: coverage?.scan_supported ?? false,
          executionSupported: coverage?.execution_supported ?? false,
          productionEnabled: readiness?.production_enabled ?? false,
          avgResponseTimeMs: health?.avg_response_time_ms ?? null,
          uptimePercent: health?.uptime_percent ?? null,
          consecutiveFailures: health?.consecutive_failures ?? 0,
          eventsParsed: health?.events_parsed ?? 0,
          lastSuccess: health?.last_success ?? null,
          lastError: health?.last_error ?? null,
          diagnostics,
          readinessChecks,
          rootCause: deriveRootCause(health?.last_error ?? null, diagnostics, coverage?.notes ?? null),
          operatorAction: getOperatorAction({
            runtimeStatus,
            readinessStage,
            consecutiveFailures: health?.consecutive_failures ?? 0,
            lastError: health?.last_error ?? null,
            diagnostics,
            productionEnabled: readiness?.production_enabled ?? false,
          }),
          triageLane,
          attentionScore,
          runtimeIssue,
          readinessIssue,
        } satisfies MergedParserRow
      })
      .filter((entry): entry is MergedParserRow => Boolean(entry))
      .sort((a, b) => b.attentionScore - a.attentionScore || a.name.localeCompare(b.name))
  }, [parserCoverage, parserHealth])

  const diagnosticCodes = useMemo(() => {
    return [...new Set(rows.flatMap((entry) => entry.diagnostics.map((check) => check.code)))]
      .sort((a, b) => a.localeCompare(b))
  }, [rows])

  const filteredRows = useMemo(() => {
    return rows.filter((entry) => {
      if (!matchesTopFilter(entry, filter)) return false

      if (runtimeFilter !== 'all') {
        const rowRuntime = entry.runtimeStatus ?? 'unknown'
        if (rowRuntime !== runtimeFilter) return false
      }

      if (readinessFilter !== 'all' && entry.readinessStage !== readinessFilter) return false
      if (severityFilter !== 'all' && !entry.diagnostics.some((check) => check.severity === severityFilter)) return false
      if (codeFilter !== 'all' && !entry.diagnostics.some((check) => check.code === codeFilter)) return false

      return true
    })
  }, [codeFilter, filter, readinessFilter, rows, runtimeFilter, severityFilter])

  const summary = useMemo(() => {
    const uptimeSamples = rows.flatMap((entry) => entry.uptimePercent !== null ? [entry.uptimePercent] : [])
    const latencySamples = rows.flatMap((entry) => entry.avgResponseTimeMs !== null ? [entry.avgResponseTimeMs] : [])
    const failChecks = rows.flatMap((entry) => entry.diagnostics.filter((check) => check.severity === 'fail'))
    const warnChecks = rows.flatMap((entry) => entry.diagnostics.filter((check) => check.severity === 'warn'))

    return {
      total: rows.length,
      runtimeClean: rows.filter((entry) => entry.runtimeStatus === 'Healthy').length,
      incidents: rows.filter((entry) => entry.runtimeStatus === 'Unhealthy' || entry.runtimeStatus === 'CircuitOpen').length,
      readinessBlocked: rows.filter((entry) => entry.readinessStage === 'blocked').length,
      readinessWatch: rows.filter((entry) => entry.readinessStage === 'diagnostic_only' || entry.readinessStage === 'rollout_ready').length,
      avgUptime: average(uptimeSamples),
      avgLatency: average(latencySamples),
      failChecks: failChecks.length,
      warnChecks: warnChecks.length,
    }
  }, [rows])

  const leadingSignals = useMemo(() => filteredRows.slice(0, 3), [filteredRows])

  const diagnosticsLeaderboard = useMemo(() => {
    const counts = new Map<string, { code: string, severity: BackendDiagnosticSeverity, message: string, count: number }>()

    filteredRows.forEach((entry) => {
      entry.diagnostics.forEach((check) => {
        const key = `${check.code}:${check.severity}:${check.message}`
        const current = counts.get(key)
        if (current) {
          current.count += 1
          return
        }
        counts.set(key, { ...check, count: 1 })
      })
    })

    return [...counts.values()]
      .sort((a, b) => {
        const severityWeight = (severity: BackendDiagnosticSeverity) => {
          switch (severity) {
            case 'fail': return 4
            case 'warn': return 3
            case 'info': return 2
            case 'pass': return 1
          }
        }

        return severityWeight(b.severity) - severityWeight(a.severity) || b.count - a.count || a.code.localeCompare(b.code)
      })
      .slice(0, 8)
  }, [filteredRows])

  const triageSummary = useMemo(() => {
    return {
      critical: rows.filter((entry) => entry.triageLane === 'critical').length,
      stabilize: rows.filter((entry) => entry.triageLane === 'stabilize').length,
      rollout: rows.filter((entry) => entry.triageLane === 'rollout').length,
      observe: rows.filter((entry) => entry.triageLane === 'observe').length,
    }
  }, [rows])

  const triageQueue = useMemo(() => filteredRows.slice(0, 5), [filteredRows])

  const activeDetailFilters = severityFilter !== 'all' || codeFilter !== 'all' || runtimeFilter !== 'all' || readinessFilter !== 'all'

  const filterLabel = `${filteredRows.length} / ${rows.length} parsers`
  const parserBrief = useMemo(() => {
    const topQueue = triageQueue[0] ?? null
    const topSignal = leadingSignals[0] ?? null

    return {
      tone: triageSummary.critical > 0
        ? 'danger'
        : triageSummary.stabilize > 0 || summary.failChecks > 0
          ? 'warning'
          : 'success',
      summary: rows.length > 0
        ? `${triageSummary.critical + triageSummary.stabilize} parsers currently need operator eyes. ${topQueue ? `${topQueue.name} leads the queue with ${topQueue.attentionScore} pressure points.` : 'Deep-dive is ready for the next snapshot.'}`
        : 'Parser deep-dive is waiting for health and coverage snapshots.',
      pills: [
        { label: 'critical', value: `${triageSummary.critical}`, tone: triageSummary.critical > 0 ? 'danger' : 'success' },
        { label: 'stabilize', value: `${triageSummary.stabilize}`, tone: triageSummary.stabilize > 0 ? 'warning' : 'success' },
        { label: 'fail checks', value: `${summary.failChecks}`, tone: summary.failChecks > 0 ? 'danger' : 'success' },
        { label: 'avg latency', value: `${summary.avgLatency.toFixed(0)} ms`, tone: summary.avgLatency > 1500 ? 'warning' : 'info' },
      ],
      actions: [
        topQueue ? `${topQueue.name}: ${topQueue.operatorAction}` : 'No parser is currently queued for intervention.',
        topSignal ? `${topSignal.name}: ${topSignal.rootCause}` : 'No runtime or readiness signal is active right now.',
      ],
    } as const
  }, [leadingSignals, rows.length, summary.avgLatency, summary.failChecks, triageQueue, triageSummary.critical, triageSummary.stabilize])

  return (
    <div className="glass-card p-5">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between mb-5">
        <div>
          <h3 className="text-base font-semibold">Parser health deep dive</h3>
          <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
            Root-cause surface over `/api/v1/parsers/health` + `/api/v1/parsers/coverage`
          </p>
        </div>

        <div className="flex flex-wrap gap-2">
          {([
            ['attention', `attention ${rows.filter((entry) => entry.runtimeIssue || entry.readinessIssue).length}`, 'badge-danger'],
            ['runtime', `runtime ${rows.filter((entry) => entry.runtimeIssue).length}`, 'badge-warning'],
            ['readiness', `readiness ${rows.filter((entry) => entry.readinessIssue).length}`, 'badge-info'],
            ['all', `all ${rows.length}`, 'badge-success'],
          ] as const).map(([key, label, badge]) => (
            <button
              key={key}
              type="button"
              onClick={() => setFilter(key)}
              className={`badge transition-all ${filter === key ? badge : 'badge-info opacity-70'}`}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="mb-5">
        <CompactSignalOverlay
          title="Parser snapshot brief"
          tone={parserBrief.tone}
          summary={parserBrief.summary}
          pills={parserBrief.pills}
          actions={parserBrief.actions}
        />
      </div>

      <div className="rounded-xl p-4 mb-5" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
        <div className="flex flex-col gap-4">
          <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
            <div>
              <h4 className="text-sm font-semibold">Diagnostics filters</h4>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                Narrow parser deep-dive by runtime, readiness, severity and diagnostic code
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <span className="badge badge-info">{filterLabel}</span>
              {activeDetailFilters && (
                <button
                  type="button"
                  onClick={() => {
                    setSeverityFilter('all')
                    setCodeFilter('all')
                    setRuntimeFilter('all')
                    setReadinessFilter('all')
                  }}
                  className="badge badge-warning transition-all"
                >
                  reset filters
                </button>
              )}
            </div>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
            <label className="block">
              <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>Severity</span>
              <select value={severityFilter} onChange={(event) => setSeverityFilter(event.target.value as SeverityFilter)} className="input mt-2">
                <option value="all">All severities</option>
                <option value="fail">Fail</option>
                <option value="warn">Warn</option>
                <option value="info">Info</option>
                <option value="pass">Pass</option>
              </select>
            </label>

            <label className="block">
              <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>Diagnostic code</span>
              <select value={codeFilter} onChange={(event) => setCodeFilter(event.target.value)} className="input mt-2">
                <option value="all">All codes</option>
                {diagnosticCodes.map((code) => (
                  <option key={code} value={code}>{code}</option>
                ))}
              </select>
            </label>

            <label className="block">
              <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>Runtime</span>
              <select value={runtimeFilter} onChange={(event) => setRuntimeFilter(event.target.value as RuntimeFilter)} className="input mt-2">
                <option value="all">All runtime states</option>
                <option value="Healthy">Healthy</option>
                <option value="Degraded">Degraded</option>
                <option value="Unhealthy">Unhealthy</option>
                <option value="CircuitOpen">Circuit open</option>
                <option value="unknown">Unknown</option>
              </select>
            </label>

            <label className="block">
              <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-secondary)' }}>Readiness</span>
              <select value={readinessFilter} onChange={(event) => setReadinessFilter(event.target.value as ReadinessFilter)} className="input mt-2">
                <option value="all">All readiness stages</option>
                <option value="production">Production</option>
                <option value="rollout_ready">Rollout ready</option>
                <option value="diagnostic_only">Diagnostic only</option>
                <option value="blocked">Blocked</option>
              </select>
            </label>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-2 xl:grid-cols-5 gap-3 mb-5">
        {[
          { label: 'Runtime clean', value: `${summary.runtimeClean} / ${summary.total}`, detail: `${summary.incidents} incidents`, icon: ShieldCheck, color: 'var(--accent-green)' },
          { label: 'Blocked readiness', value: `${summary.readinessBlocked}`, detail: `${summary.readinessWatch} rollout/diag`, icon: ShieldX, color: 'var(--accent-red)' },
          { label: 'Avg uptime', value: `${summary.avgUptime.toFixed(1)}%`, detail: 'health snapshot', icon: Activity, color: 'var(--accent-cyan)' },
          { label: 'Avg response', value: `${summary.avgLatency.toFixed(0)} ms`, detail: 'parser roundtrip', icon: TimerReset, color: 'var(--accent-blue)' },
          { label: 'Diagnostics', value: `${summary.failChecks} fail`, detail: `${summary.warnChecks} warn`, icon: Siren, color: 'var(--accent-yellow)' },
        ].map((entry) => {
          const Icon = entry.icon
          return (
            <div key={entry.label} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <div className="flex items-center justify-between mb-3">
                <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{entry.label}</span>
                <Icon size={16} style={{ color: entry.color }} />
              </div>
              <p className="text-xl font-semibold">{entry.value}</p>
              <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.detail}</p>
            </div>
          )
        })}
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[1.2fr,1.4fr] gap-5 mb-5">
        <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
          <div className="flex items-center justify-between mb-3 gap-4">
            <div>
              <h4 className="text-sm font-semibold">Triage lanes</h4>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Fast queue split for operator focus</p>
            </div>
            <Wrench size={16} style={{ color: 'var(--accent-cyan)' }} />
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            {[
              {
                lane: 'critical' as const,
                count: triageSummary.critical,
                detail: 'Runtime down, breaker open or fail diagnostics',
                badge: 'badge-danger',
                onClick: () => setFilter('attention'),
              },
              {
                lane: 'stabilize' as const,
                count: triageSummary.stabilize,
                detail: 'Warnings, degraded runtime or blocked readiness',
                badge: 'badge-warning',
                onClick: () => setFilter('attention'),
              },
              {
                lane: 'rollout' as const,
                count: triageSummary.rollout,
                detail: 'Ready to graduate once gates are confirmed',
                badge: 'badge-info',
                onClick: () => {
                  setFilter('readiness')
                  setReadinessFilter('rollout_ready')
                },
              },
              {
                lane: 'observe' as const,
                count: triageSummary.observe,
                detail: 'Healthy production parsers with no active signal',
                badge: 'badge-success',
                onClick: () => setFilter('all'),
              },
            ].map((entry) => (
              <button
                key={entry.lane}
                type="button"
                onClick={entry.onClick}
                className="rounded-xl p-4 text-left transition-all hover:-translate-y-0.5"
                style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}
              >
                <div className="flex items-center justify-between gap-3 mb-3">
                  <span className={`badge ${entry.badge}`}>{formatTriageLane(entry.lane)}</span>
                  <span className="text-lg font-semibold">{entry.count}</span>
                </div>
                <p className="text-sm">{entry.detail}</p>
              </button>
            ))}
          </div>
        </div>

        <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
          <div className="flex items-center justify-between mb-3 gap-4">
            <div>
              <h4 className="text-sm font-semibold">Operator queue</h4>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Highest-pressure parsers with next action</p>
            </div>
            <Eye size={16} style={{ color: 'var(--accent-blue)' }} />
          </div>

          <div className="space-y-3">
            {triageQueue.map((entry) => (
              <div key={`triage-${entry.slug}`} className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid var(--border-color)' }}>
                <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-medium">{entry.name}</p>
                      <span className={`badge ${triageBadgeClass(entry.triageLane)}`}>{formatTriageLane(entry.triageLane)}</span>
                      <span className={`badge ${healthBadgeClass(entry.runtimeStatus)}`}>{formatHealthStatus(entry.runtimeStatus)}</span>
                    </div>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.rootCause}</p>
                    <p className="text-sm mt-2">{entry.operatorAction}</p>
                  </div>

                  <div className="text-right shrink-0">
                    <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Pressure</p>
                    <p className="text-lg font-semibold">{entry.attentionScore}</p>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.consecutiveFailures} fails • {formatDurationFromNow(entry.lastSuccess)}</p>
                  </div>
                </div>
              </div>
            ))}

            {triageQueue.length === 0 && (
              <div className="text-center py-10" style={{ color: 'var(--text-muted)' }}>
                <CheckCircle2 size={36} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Triage queue появится после первого parser snapshot</p>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[1.65fr,1fr] gap-5 mb-5">
        <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
          <div className="flex items-center justify-between mb-3 gap-4">
            <div>
              <h4 className="text-sm font-semibold">Immediate signals</h4>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Top parsers ranked by runtime + readiness pressure</p>
            </div>
            <AlertTriangle size={16} style={{ color: 'var(--accent-yellow)' }} />
          </div>

          <div className="space-y-3">
            {leadingSignals.map((entry) => (
              <div key={entry.slug} className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid var(--border-color)' }}>
                <div className="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-medium">{entry.name}</p>
                      <span className={`badge ${healthBadgeClass(entry.runtimeStatus)}`}>{formatHealthStatus(entry.runtimeStatus)}</span>
                      <span className={`badge ${readinessBadgeClass(entry.readinessStage)}`}>{formatReadinessStage(entry.readinessStage)}</span>
                    </div>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                      {entry.parserType ?? 'unknown'} parser • {entry.executionSupported ? 'execution path' : 'scan only'} • last success {formatDurationFromNow(entry.lastSuccess)}
                    </p>
                    <p className="text-sm mt-2">{entry.rootCause}</p>
                  </div>

                  <div className="grid grid-cols-2 gap-2 min-w-[220px] text-sm">
                    <div className="rounded-lg px-3 py-2" style={{ background: 'var(--bg-card)' }}>
                      <p style={{ color: 'var(--text-muted)' }}>Uptime</p>
                      <p className="font-semibold">{entry.uptimePercent !== null ? `${entry.uptimePercent.toFixed(1)}%` : '—'}</p>
                    </div>
                    <div className="rounded-lg px-3 py-2" style={{ background: 'var(--bg-card)' }}>
                      <p style={{ color: 'var(--text-muted)' }}>Resp</p>
                      <p className="font-semibold">{entry.avgResponseTimeMs !== null ? `${entry.avgResponseTimeMs.toFixed(0)} ms` : '—'}</p>
                    </div>
                    <div className="rounded-lg px-3 py-2" style={{ background: 'var(--bg-card)' }}>
                      <p style={{ color: 'var(--text-muted)' }}>Failures</p>
                      <p className="font-semibold">{entry.consecutiveFailures}</p>
                    </div>
                    <div className="rounded-lg px-3 py-2" style={{ background: 'var(--bg-card)' }}>
                      <p style={{ color: 'var(--text-muted)' }}>Events</p>
                      <p className="font-semibold">{entry.eventsParsed}</p>
                    </div>
                  </div>
                </div>
              </div>
            ))}

            {leadingSignals.length === 0 && (
              <div className="text-center py-10" style={{ color: 'var(--text-muted)' }}>
                <CheckCircle2 size={36} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Deep-dive появится после первого parser snapshot</p>
              </div>
            )}
          </div>
        </div>

        <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
          <div className="flex items-center justify-between mb-3 gap-4">
            <div>
              <h4 className="text-sm font-semibold">Diagnostics cluster</h4>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Повторяющиеся коды из readiness и health</p>
            </div>
            <Radar size={16} style={{ color: 'var(--accent-blue)' }} />
          </div>

          <div className="space-y-2">
            {diagnosticsLeaderboard.map((entry) => (
              <div key={`${entry.code}-${entry.message}`} className="rounded-lg p-3" style={{ background: 'rgba(255,255,255,0.02)', border: '1px solid var(--border-color)' }}>
                <div className="flex items-center justify-between gap-3 mb-1">
                  <span className={`badge ${badgeClassBySeverity(entry.severity)}`}>{entry.severity}</span>
                  <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>{entry.count} parsers</span>
                </div>
                <p className="text-xs font-mono" style={{ color: 'var(--text-secondary)' }}>{entry.code}</p>
                <p className="text-sm mt-1">{entry.message}</p>
              </div>
            ))}

            {diagnosticsLeaderboard.length === 0 && (
              <div className="text-center py-10" style={{ color: 'var(--text-muted)' }}>
                <Gauge size={36} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Diagnostics payload пока пустой</p>
              </div>
            )}
          </div>
        </div>
      </div>

      <div className="rounded-xl overflow-auto" style={{ border: '1px solid var(--border-color)' }}>
        <div className="min-w-[980px]">
          <div className="grid grid-cols-[minmax(210px,1.5fr),minmax(180px,1.05fr),110px,110px,90px,minmax(220px,2fr)] gap-3 px-4 py-3 text-xs uppercase tracking-wider" style={{ background: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}>
            <div>Parser</div>
            <div>Runtime / readiness</div>
            <div>Uptime</div>
            <div>Resp time</div>
            <div>Fails</div>
            <div>Root cause / diagnostics</div>
          </div>

          <div>
          {filteredRows.map((entry) => {
            const matchingChecks = entry.diagnostics.filter((check) => {
              if (severityFilter !== 'all' && check.severity !== severityFilter) return false
              if (codeFilter !== 'all' && check.code !== codeFilter) return false
              return check.severity !== 'pass' || severityFilter === 'pass'
            })
            const visibleChecks = (matchingChecks.length > 0 ? matchingChecks : entry.diagnostics.filter((check) => check.severity !== 'pass')).slice(0, 3)

            return (
              <div key={entry.slug} className="grid grid-cols-[minmax(210px,1.5fr),minmax(180px,1.05fr),110px,110px,90px,minmax(220px,2fr)] gap-3 px-4 py-4 border-t" style={{ borderColor: 'var(--border-color)', background: 'var(--bg-card)' }}>
                <div>
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="text-sm font-medium">{entry.name}</p>
                    <span className={`badge ${triageBadgeClass(entry.triageLane)}`}>{formatTriageLane(entry.triageLane)}</span>
                    {entry.executionSupported ? <span className="badge badge-success">exec</span> : <span className="badge badge-info">scan</span>}
                  </div>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                    {entry.parserType ?? 'unknown'} • {entry.source ?? 'source n/a'}
                  </p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>
                    last success {formatDateTime(entry.lastSuccess)}
                  </p>
                </div>

                <div>
                  <div className="flex flex-wrap gap-2 mb-2">
                    <span className={`badge ${healthBadgeClass(entry.runtimeStatus)}`}>{formatHealthStatus(entry.runtimeStatus)}</span>
                    <span className={`badge ${readinessBadgeClass(entry.readinessStage)}`}>{formatReadinessStage(entry.readinessStage)}</span>
                  </div>
                  <p className="text-xs" style={{ color: 'var(--text-secondary)' }}>
                    {entry.enabled ? 'enabled' : 'disabled'} • {entry.productionEnabled ? 'prod enabled' : 'prod gated'}
                  </p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>
                    {entry.scanSupported ? 'scan ready' : 'scan off'} • {entry.executionSupported ? 'execution ready' : 'execution off'}
                  </p>
                </div>

                <div className="text-sm">
                  <p className="font-semibold">{entry.uptimePercent !== null ? `${entry.uptimePercent.toFixed(1)}%` : '—'}</p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>{entry.eventsParsed} events</p>
                </div>

                <div className="text-sm">
                  <p className="font-semibold">{entry.avgResponseTimeMs !== null ? `${entry.avgResponseTimeMs.toFixed(0)} ms` : '—'}</p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>{formatDurationFromNow(entry.lastSuccess)}</p>
                </div>

                <div className="text-sm">
                  <p className={`font-semibold ${entry.consecutiveFailures > 0 ? 'profit-negative' : ''}`}>{entry.consecutiveFailures}</p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>{entry.readinessChecks.length} checks</p>
                </div>

                <div>
                  <p className="text-sm">{entry.rootCause}</p>
                  <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>{entry.operatorAction}</p>
                  <div className="flex flex-wrap gap-2 mt-2">
                    {visibleChecks.map((check) => (
                      <span key={`${entry.slug}-${check.code}-${check.message}`} className={`badge ${badgeClassBySeverity(check.severity)}`} title={check.message}>
                        {check.code}
                      </span>
                    ))}
                    {visibleChecks.length === 0 && <span className="badge badge-success">no active diagnostics</span>}
                  </div>
                </div>
              </div>
            )
          })}

            {filteredRows.length === 0 && (
              <div className="py-12 text-center" style={{ background: 'var(--bg-card)', color: 'var(--text-muted)' }}>
                <ShieldAlert size={40} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">По текущему фильтру нет parser signals</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
