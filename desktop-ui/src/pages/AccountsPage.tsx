import { useEffect, useMemo, useState } from 'react'
import { motion } from 'framer-motion'
import { AlertTriangle, ArrowDownLeft, ArrowUpRight, BadgeAlert, CircleDollarSign, Landmark, ShieldAlert, ShieldCheck, ShieldQuestion, Wallet, X } from 'lucide-react'
import type {
  AccountSessionSummary,
  AccountStateResponse,
  BankrollRecommendationsResponse,
  BankrollState,
  DepositAllocationTarget,
} from '../types'

interface AccountsPageProps {
  accounts: AccountStateResponse[]
  accountsSummary: AccountSessionSummary | null
  bankrollState: BankrollState | null
  bankrollRecommendations: BankrollRecommendationsResponse | null
  focusedBookmaker: string | null
}

type NormalizedUrgency = 'high' | 'medium' | 'low'

const container = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { staggerChildren: 0.05 } },
}

const item = {
  hidden: { opacity: 0, y: 16 },
  show: { opacity: 1, y: 0, transition: { duration: 0.28 } },
}

function formatMoney(value: number) {
  return `${value >= 0 ? '+' : ''}${value.toLocaleString('ru-RU', { maximumFractionDigits: 0 })} RUB`
}

function formatCompactMoney(value: number) {
  return `${Math.round(value).toLocaleString('ru-RU')} RUB`
}

function formatPercent(value: number) {
  return `${value.toFixed(0)}%`
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

function normalizeUrgency(value: DepositAllocationTarget['urgency']): NormalizedUrgency {
  const normalized = String(value).toLowerCase()
  if (normalized === 'high') return 'high'
  if (normalized === 'medium') return 'medium'
  return 'low'
}

function urgencyBadgeClass(value: DepositAllocationTarget['urgency'] | NormalizedUrgency) {
  switch (normalizeUrgency(value)) {
    case 'high':
      return 'badge-danger'
    case 'medium':
      return 'badge-warning'
    default:
      return 'badge-success'
  }
}

function urgencyWeight(value: DepositAllocationTarget['urgency'] | NormalizedUrgency) {
  switch (normalizeUrgency(value)) {
    case 'high':
      return 3
    case 'medium':
      return 2
    default:
      return 1
  }
}

function readinessLabel(account: AccountStateResponse | null) {
  if (!account) return 'No account'
  if (account.readiness.placement_ready) return 'Placement ready'
  if (account.readiness.session_ready && account.readiness.balance_ready) return 'Needs operator review'
  if (!account.readiness.session_ready) return 'Session restore needed'
  if (!account.readiness.balance_ready) return 'Balance refresh needed'
  return 'Blocked'
}

function sessionTone(sessionState: string | null | undefined) {
  const normalized = String(sessionState ?? '').toLowerCase()
  if (!normalized) return 'badge-warning'
  if (normalized.includes('auth') || normalized.includes('ready') || normalized.includes('active')) return 'badge-success'
  if (normalized.includes('expired') || normalized.includes('fail') || normalized.includes('error')) return 'badge-danger'
  return 'badge-warning'
}

function readinessScore(account: AccountStateResponse, depositGap: number, available: number) {
  let score = 0
  if (!account.readiness.placement_ready) score += 45
  if (!account.readiness.session_ready) score += 20
  if (!account.readiness.balance_ready) score += 15
  score += Math.min(account.control_issues.length * 12, 24)
  score += Math.min(account.readiness.blocking_reasons.length * 8, 16)
  if (account.readiness.submit_blocked_by_safe_mode) score += 12
  if (account.readiness.approval_required) score += 8
  if (depositGap > 0) score += Math.min((depositGap / Math.max(available || 1, 1)) * 20, 20)
  return Math.round(score)
}

function heatmapTone(coverage: number) {
  if (coverage >= 1.2) return 'rgba(63, 185, 80, 0.18)'
  if (coverage >= 0.85) return 'rgba(210, 153, 34, 0.18)'
  return 'rgba(248, 81, 73, 0.18)'
}

export function AccountsPage({ accounts, accountsSummary, bankrollState, bankrollRecommendations, focusedBookmaker }: AccountsPageProps) {
  const [selectedBookmaker, setSelectedBookmaker] = useState<string | null>(null)
  const accountMap = useMemo(() => new Map(accounts.map((entry) => [entry.bookmaker.toLowerCase(), entry])), [accounts])
  const guidanceMap = useMemo(() => new Map((bankrollRecommendations?.deposit_guidance.targets ?? []).map((entry) => [entry.bookmaker.toLowerCase(), entry])), [bankrollRecommendations])

  const bankrollRows = useMemo(() => {
    return (bankrollState?.bookmakers ?? [])
      .map((entry) => {
        const account = accountMap.get(entry.bookmaker.toLowerCase()) ?? null
        const guidance = guidanceMap.get(entry.bookmaker.toLowerCase()) ?? null
        const exposureRatio = entry.balance > 0 ? entry.exposure / entry.balance : 0
        const depositGap = guidance?.deposit_gap ?? Math.max(entry.recommended_deposit, 0)
        const riskScore = account ? readinessScore(account, depositGap, entry.available) : Math.round(exposureRatio * 100)

        return {
          ...entry,
          account,
          guidance,
          exposureRatio,
          depositGap,
          riskScore,
        }
      })
      .sort((a, b) => b.riskScore - a.riskScore || b.depositGap - a.depositGap || b.exposureRatio - a.exposureRatio)
  }, [accountMap, bankrollState, guidanceMap])

  const executionBlockers = useMemo(() => {
    const blockedRows = bankrollRows.filter((entry) => entry.account && !entry.account.readiness.placement_ready)
    const blockerMap = new Map<string, { label: string, count: number, available: number }>()

    blockedRows.forEach((entry) => {
      const account = entry.account
      if (!account) return

      const reasons = [
        ...(account.readiness.operator_action ? [account.readiness.operator_action] : []),
        ...account.readiness.blocking_reasons,
        ...account.control_issues,
      ]

      Array.from(new Set(reasons.filter(Boolean))).forEach((reason) => {
        const current = blockerMap.get(reason)
        blockerMap.set(reason, {
          label: reason,
          count: (current?.count ?? 0) + 1,
          available: (current?.available ?? 0) + entry.available,
        })
      })
    })

    return {
      blockedAccounts: blockedRows.length,
      strandedLiquidity: blockedRows.reduce((sum, entry) => sum + entry.available, 0),
      safeMode: blockedRows.filter((entry) => entry.account?.readiness.submit_blocked_by_safe_mode).length,
      approval: blockedRows.filter((entry) => entry.account?.readiness.approval_required).length,
      sessionRestore: blockedRows.filter((entry) => entry.account && !entry.account.readiness.session_ready).length,
      blockers: Array.from(blockerMap.values())
        .sort((a, b) => b.count - a.count || b.available - a.available || a.label.localeCompare(b.label))
        .slice(0, 4),
    }
  }, [bankrollRows])

  const summary = accountsSummary ?? {
    total_bookmakers: accounts.length,
    accounts_configured: accounts.filter((entry) => entry.account).length,
    accounts_enabled: accounts.filter((entry) => entry.account?.enabled).length,
    disabled_accounts: accounts.filter((entry) => entry.account && !entry.account.enabled).length,
    accounts_with_control_issues: accounts.filter((entry) => entry.control_issues.length > 0).length,
    sessions_configured: accounts.filter((entry) => entry.session).length,
    sessions_authenticated: accounts.filter((entry) => entry.readiness.session_ready).length,
    balances_cached: accounts.filter((entry) => entry.balance).length,
    ready_for_execution: accounts.filter((entry) => entry.readiness.placement_ready).length,
    ready_for_dry_run: accounts.filter((entry) => entry.readiness.dry_run_ready).length,
  }

  const readinessRate = summary.total_bookmakers > 0 ? (summary.ready_for_execution / summary.total_bookmakers) * 100 : 0
  const bankrollCoverage = bankrollRecommendations?.deposit_guidance.total_budget_limit
    ? (bankrollRecommendations.deposit_guidance.current_available_total / bankrollRecommendations.deposit_guidance.total_budget_limit) * 100
    : 0

  const topDepositTargets = (bankrollRecommendations?.deposit_guidance.targets ?? [])
    .filter((entry) => entry.recommended_deposit > 0)
    .sort((a, b) => urgencyWeight(b.urgency) - urgencyWeight(a.urgency) || b.deposit_gap - a.deposit_gap || b.recommended_deposit - a.recommended_deposit)
    .slice(0, 4)

  const actionQueue = bankrollRows
    .filter((entry) => entry.depositGap > 0 || (entry.account && (!entry.account.readiness.placement_ready || entry.account.control_issues.length > 0 || entry.account.readiness.blocking_reasons.length > 0)))
    .map((entry) => {
      const urgency = entry.guidance ? normalizeUrgency(entry.guidance.urgency) : entry.depositGap > 0 ? 'medium' : 'low'
      const primaryAction = entry.depositGap > 0
        ? `Top up ${formatCompactMoney(entry.depositGap)} to restore target liquidity`
        : entry.account?.readiness.operator_action
          ?? entry.account?.readiness.blocking_reasons[0]
          ?? (entry.account?.control_issues.length ? `${entry.account.control_issues.length} control issues need review` : 'Review readiness before next execution')

      return {
        ...entry,
        urgency,
        primaryAction,
      }
    })
    .sort((a, b) => urgencyWeight(b.urgency) - urgencyWeight(a.urgency) || b.riskScore - a.riskScore || b.depositGap - a.depositGap)

  const selectedEntry = useMemo(() => {
    if (!selectedBookmaker) return bankrollRows[0] ?? null
    return bankrollRows.find((entry) => entry.bookmaker === selectedBookmaker) ?? bankrollRows[0] ?? null
  }, [bankrollRows, selectedBookmaker])

  useEffect(() => {
    if (focusedBookmaker) {
      setSelectedBookmaker(focusedBookmaker)
    }
  }, [focusedBookmaker])

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="space-y-6">
      <motion.div variants={item} className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <h2 className="text-2xl font-bold">Accounts / Bankroll Readiness</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Read-only экран поверх `/api/v1/accounts`, `/api/v1/accounts/summary`, `/api/v1/bankroll` и `/api/v1/bankroll/recommendations`.
          </p>
        </div>

        <div className="flex flex-wrap gap-3 text-xs">
          <div className="rounded-xl px-3 py-2" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            Accounts snapshot {accounts.length}
          </div>
          <div className="rounded-xl px-3 py-2" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            Bankroll update {formatDateTime(bankrollState?.updated_at ?? null)}
          </div>
        </div>
      </motion.div>

      <div className="grid grid-cols-1 xl:grid-cols-4 gap-4">
        {[
          { label: 'Execution-ready', value: `${summary.ready_for_execution} / ${summary.total_bookmakers}`, detail: formatPercent(readinessRate), icon: ShieldCheck, color: 'var(--accent-green)' },
          { label: 'Control issues', value: summary.accounts_with_control_issues.toString(), detail: `${summary.disabled_accounts} disabled`, icon: ShieldAlert, color: 'var(--accent-red)' },
          { label: 'Available bankroll', value: formatCompactMoney(bankrollRecommendations?.deposit_guidance.current_available_total ?? 0), detail: `coverage ${formatPercent(bankrollCoverage)}`, icon: Wallet, color: 'var(--accent-blue)' },
          { label: 'Recommended deposit', value: formatCompactMoney(bankrollRecommendations?.deposit_guidance.total_recommended_deposit ?? 0), detail: `target ${formatCompactMoney(bankrollRecommendations?.deposit_guidance.target_per_bookmaker ?? 0)}`, icon: CircleDollarSign, color: 'var(--accent-yellow)' },
        ].map((entry) => {
          const Icon = entry.icon
          return (
            <motion.div key={entry.label} variants={item} className="glass-card p-5">
              <div className="flex items-start justify-between mb-3">
                <div>
                  <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{entry.label}</p>
                  <p className="text-xl font-semibold mt-1">{entry.value}</p>
                </div>
                <Icon size={18} style={{ color: entry.color }} />
              </div>
              <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>{entry.detail}</p>
            </motion.div>
          )
        })}
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[1.2fr,0.8fr] gap-6">
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Accounts summary</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Configuration, session and dry-run readiness snapshot</p>
            </div>
            <span className="badge badge-info">read-only</span>
          </div>

          <div className="grid grid-cols-2 lg:grid-cols-5 gap-3">
            {[
              ['Configured', summary.accounts_configured],
              ['Enabled', summary.accounts_enabled],
              ['Sessions auth', summary.sessions_authenticated],
              ['Balances cached', summary.balances_cached],
              ['Dry-run ready', summary.ready_for_dry_run],
            ].map(([label, value]) => (
              <div key={label} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{label}</p>
                <p className="text-2xl font-semibold mt-2">{value}</p>
              </div>
            ))}
          </div>
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Priority action queue</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Urgency-first view across deposit gaps, blockers and control issues</p>
            </div>
            <Landmark size={16} style={{ color: 'var(--accent-cyan)' }} />
          </div>

          <div className="space-y-3">
            {actionQueue.length > 0 ? actionQueue.slice(0, 5).map((entry, index) => (
              <button
                key={entry.bookmaker}
                type="button"
                onClick={() => setSelectedBookmaker(entry.bookmaker)}
                className="w-full rounded-xl p-4 text-left transition-colors"
                style={{
                  background: selectedEntry?.bookmaker === entry.bookmaker ? 'var(--bg-hover)' : 'var(--bg-secondary)',
                  border: '1px solid var(--border-color)',
                }}
              >
                <div className="flex items-center justify-between gap-3 mb-2">
                  <div className="flex items-center gap-2">
                    <span className="badge badge-info">#{index + 1}</span>
                    <p className="text-sm font-semibold">{entry.bookmaker}</p>
                  </div>
                  <span className={`badge ${urgencyBadgeClass(entry.urgency)}`}>{entry.urgency}</span>
                </div>
                <div className="flex flex-wrap gap-2 mb-2">
                  <span className={`badge ${entry.riskScore >= 55 ? 'badge-danger' : entry.riskScore >= 30 ? 'badge-warning' : 'badge-success'}`}>risk {entry.riskScore}</span>
                  {entry.depositGap > 0 ? <span className="badge badge-warning">gap {formatCompactMoney(entry.depositGap)}</span> : null}
                  {entry.account?.control_issues.length ? <span className="badge badge-danger">{entry.account.control_issues.length} issues</span> : null}
                  {entry.account?.session ? <span className={`badge ${sessionTone(entry.account.session.state)}`}>{entry.account.session.state}</span> : <span className="badge badge-warning">no session</span>}
                </div>
                <p className="text-sm leading-6">{entry.primaryAction}</p>
                <p className="text-xs leading-5 mt-2" style={{ color: 'var(--text-muted)' }}>
                  {entry.guidance?.note ?? entry.account?.readiness.blocking_reasons[0] ?? readinessLabel(entry.account)}
                </p>
              </button>
            )) : (
              <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
                <Wallet size={24} className="mx-auto mb-3 opacity-40" />
                <p className="text-sm">Нет срочных top-up или readiness blockers по текущему snapshot.</p>
              </div>
            )}

            {topDepositTargets.length > 0 ? (
              <div className="rounded-xl p-4" style={{ background: 'rgba(88, 166, 255, 0.08)', border: '1px solid rgba(88, 166, 255, 0.18)' }}>
                <p className="text-xs uppercase tracking-wider mb-3" style={{ color: 'var(--text-muted)' }}>Top deposit focus</p>
                <div className="space-y-2">
                  {topDepositTargets.slice(0, 3).map((entry) => (
                    <div key={entry.bookmaker} className="flex items-center justify-between gap-3 text-sm">
                      <span>{entry.bookmaker}</span>
                      <span className="profit-positive">{formatMoney(entry.recommended_deposit)}</span>
                    </div>
                  ))}
                </div>
              </div>
            ) : null}
          </div>
        </motion.div>
      </div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex items-center justify-between mb-4 gap-4">
          <div>
            <h3 className="text-base font-semibold">Execution blocker digest</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              Where execution-ready coverage is lost and how much available bankroll is sitting behind blockers.
            </p>
          </div>
          <span className={`badge ${executionBlockers.blockedAccounts > 0 ? 'badge-warning' : 'badge-success'}`}>
            {executionBlockers.blockedAccounts} blocked
          </span>
        </div>

        <div className="grid grid-cols-2 xl:grid-cols-4 gap-3 mb-4">
          {[
            ['Blocked accounts', executionBlockers.blockedAccounts.toString(), 'placement not ready'],
            ['Stranded liquidity', formatCompactMoney(executionBlockers.strandedLiquidity), 'available bankroll on blocked books'],
            ['Safe-mode blocks', executionBlockers.safeMode.toString(), 'submit guard active'],
            ['Approval / session', `${executionBlockers.approval} / ${executionBlockers.sessionRestore}`, 'manual review / re-auth'],
          ].map(([label, value, detail]) => (
            <div key={String(label)} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{label}</p>
              <p className="text-2xl font-semibold mt-2">{value}</p>
              <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>{detail}</p>
            </div>
          ))}
        </div>

        {executionBlockers.blockers.length > 0 ? (
          <div className="grid gap-3 xl:grid-cols-2">
            {executionBlockers.blockers.map((entry) => (
              <div key={entry.label} className="rounded-xl p-4" style={{ background: 'rgba(210, 153, 34, 0.08)', border: '1px solid rgba(210, 153, 34, 0.18)' }}>
                <div className="flex items-center justify-between gap-3">
                  <p className="text-sm font-medium">{entry.label}</p>
                  <span className="badge badge-warning">{entry.count}</span>
                </div>
                <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>
                  Touches {formatCompactMoney(entry.available)} available bankroll across blocked bookmakers.
                </p>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
            <ShieldCheck size={24} className="mx-auto mb-3 opacity-40" />
            <p className="text-sm">Execution blockers not detected in the current accounts + bankroll snapshot.</p>
          </div>
        )}
      </motion.div>

      <div className="grid grid-cols-1 xl:grid-cols-[1.1fr,0.75fr,0.85fr] gap-6">
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Bankroll heatmap / breakdown</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Available liquidity, exposure pressure and bankroll actions per bookmaker</p>
            </div>
            <span className="badge badge-info">{bankrollRows.length} books</span>
          </div>

          <div className="grid gap-3 md:grid-cols-2">
            {bankrollRows.map((entry) => {
              const coverage = entry.balance > 0 ? entry.available / Math.max(entry.exposure, 1) : 0
              return (
                <button
                  key={entry.bookmaker}
                  type="button"
                  onClick={() => setSelectedBookmaker(entry.bookmaker)}
                  className="w-full rounded-xl p-4 text-left transition-colors"
                  style={{
                    background: selectedEntry?.bookmaker === entry.bookmaker ? 'var(--bg-hover)' : heatmapTone(coverage),
                    border: '1px solid var(--border-color)',
                  }}
                >
                  <div className="flex items-start justify-between gap-3 mb-3">
                    <div>
                      <div className="flex flex-wrap items-center gap-2">
                        <p className="text-sm font-semibold">{entry.bookmaker}</p>
                        {entry.account?.readiness.placement_ready ? <span className="badge badge-success">placement ready</span> : <span className="badge badge-warning">watch / blocked</span>}
                      </div>
                      <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                        Exposure {formatPercent(entry.exposureRatio * 100)} of balance
                      </p>
                    </div>
                    <span className={`badge ${entry.riskScore >= 55 ? 'badge-danger' : entry.riskScore >= 30 ? 'badge-warning' : 'badge-success'}`}>risk {entry.riskScore}</span>
                  </div>

                  <div className="space-y-2 text-sm">
                    <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Balance</span><span>{formatCompactMoney(entry.balance)}</span></div>
                    <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Available</span><span>{formatCompactMoney(entry.available)}</span></div>
                    <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Exposure</span><span>{formatCompactMoney(entry.exposure)}</span></div>
                    <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Coverage ratio</span><span>{coverage.toFixed(2)}x</span></div>
                  </div>

                  {(entry.recommended_deposit > 0 || entry.recommended_withdraw > 0) ? (
                    <div className="flex flex-wrap gap-2 mt-3">
                      {entry.recommended_deposit > 0 ? <span className="badge badge-warning"><ArrowUpRight size={12} /> {formatMoney(entry.recommended_deposit)}</span> : null}
                      {entry.recommended_withdraw > 0 ? <span className="badge badge-info"><ArrowDownLeft size={12} /> {formatMoney(entry.recommended_withdraw)}</span> : null}
                    </div>
                  ) : null}
                </button>
              )
            })}
          </div>
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Risk-first bookmaker list</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Sorted by execution blockers, control issues, deposit gap and exposure</p>
            </div>
            <BadgeAlert size={16} style={{ color: 'var(--accent-red)' }} />
          </div>

          <div className="space-y-3">
            {bankrollRows.map((entry, index) => (
              <button
                key={entry.bookmaker}
                type="button"
                onClick={() => setSelectedBookmaker(entry.bookmaker)}
                className="w-full rounded-xl p-4 text-left transition-colors"
                style={{
                  background: selectedEntry?.bookmaker === entry.bookmaker ? 'var(--bg-hover)' : 'var(--bg-secondary)',
                  border: '1px solid var(--border-color)',
                }}
              >
                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="badge badge-info">#{index + 1}</span>
                      <p className="text-sm font-semibold">{entry.bookmaker}</p>
                      {entry.account?.readiness.placement_ready ? <span className="badge badge-success">ready</span> : <span className="badge badge-danger">blocked</span>}
                      {entry.guidance ? <span className={`badge ${urgencyBadgeClass(entry.guidance.urgency)}`}>{String(entry.guidance.urgency).toLowerCase()}</span> : null}
                    </div>
                    <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>
                      {entry.account?.control_issues.length ? `${entry.account.control_issues.length} control issues` : 'No control issues'}
                      {entry.guidance?.deposit_gap ? ` • deposit gap ${formatCompactMoney(entry.guidance.deposit_gap)}` : ''}
                      {entry.account?.session ? ` • session ${entry.account.session.state}` : ' • no session'}
                    </p>
                    {entry.account?.readiness.operator_action ? (
                      <p className="text-xs mt-2" style={{ color: 'var(--accent-yellow)' }}>{entry.account.readiness.operator_action}</p>
                    ) : null}
                    {(entry.account?.readiness.blocking_reasons[0] || entry.guidance?.note) ? (
                      <p className="text-xs mt-2" style={{ color: 'var(--text-muted)' }}>{entry.account?.readiness.blocking_reasons[0] ?? entry.guidance?.note}</p>
                    ) : null}
                  </div>

                  <div className="text-right shrink-0">
                    <p className={`text-sm font-semibold ${entry.riskScore >= 55 ? 'profit-negative' : entry.riskScore >= 30 ? '' : 'profit-positive'}`}>risk {entry.riskScore}</p>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>balance {formatCompactMoney(entry.balance)}</p>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>available {formatCompactMoney(entry.available)}</p>
                  </div>
                </div>
              </button>
            ))}

            {bankrollRows.length === 0 ? (
              <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
                <AlertTriangle size={24} className="mx-auto mb-3 opacity-40" />
                <p className="text-sm">Нет bankroll snapshot: страница ждёт live ответ от backend GET endpoints.</p>
              </div>
            ) : null}
          </div>
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Bookmaker drill-down</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Focused read-only detail for the selected bookmaker</p>
            </div>
            <div className="flex items-center gap-2">
              {focusedBookmaker && selectedEntry?.bookmaker === focusedBookmaker ? <span className="badge badge-info">focused from operator</span> : null}
              {selectedEntry ? (
              <button
                type="button"
                onClick={() => setSelectedBookmaker(null)}
                className="badge badge-info"
              >
                <X size={12} /> reset
              </button>
              ) : null}
            </div>
          </div>

          {selectedEntry ? (
            <div className="space-y-4">
              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex items-start justify-between gap-3 mb-3">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-lg font-semibold">{selectedEntry.bookmaker}</p>
                      <span className={`badge ${selectedEntry.riskScore >= 55 ? 'badge-danger' : selectedEntry.riskScore >= 30 ? 'badge-warning' : 'badge-success'}`}>risk {selectedEntry.riskScore}</span>
                    </div>
                    <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>{readinessLabel(selectedEntry.account)}</p>
                  </div>
                  {selectedEntry.account?.readiness.placement_ready ? <ShieldCheck size={18} style={{ color: 'var(--accent-green)' }} /> : <ShieldQuestion size={18} style={{ color: 'var(--accent-yellow)' }} />}
                </div>

                <div className="grid grid-cols-2 gap-3 text-sm">
                  <div><p style={{ color: 'var(--text-muted)' }}>Balance</p><p className="mt-1 font-semibold">{formatCompactMoney(selectedEntry.balance)}</p></div>
                  <div><p style={{ color: 'var(--text-muted)' }}>Available</p><p className="mt-1 font-semibold">{formatCompactMoney(selectedEntry.available)}</p></div>
                  <div><p style={{ color: 'var(--text-muted)' }}>Exposure</p><p className="mt-1 font-semibold">{formatCompactMoney(selectedEntry.exposure)}</p></div>
                  <div><p style={{ color: 'var(--text-muted)' }}>Deposit gap</p><p className="mt-1 font-semibold">{formatCompactMoney(selectedEntry.depositGap)}</p></div>
                </div>
              </div>

              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <p className="text-xs uppercase tracking-wider mb-3" style={{ color: 'var(--text-muted)' }}>Next actions</p>
                <div className="space-y-2 text-sm">
                  <p>{selectedEntry.depositGap > 0 ? `Deposit ${formatCompactMoney(selectedEntry.depositGap)} to reach backend target.` : 'No deposit required from current guidance.'}</p>
                  <p>{selectedEntry.account?.readiness.operator_action ?? selectedEntry.guidance?.note ?? 'No explicit operator action from backend.'}</p>
                  <p style={{ color: 'var(--text-secondary)' }}>
                    Session {selectedEntry.account?.session ? selectedEntry.account.session.state : 'missing'}
                    {' • '}
                    {selectedEntry.account?.readiness.submit_blocked_by_safe_mode ? 'safe mode blocks submit' : 'no safe mode block'}
                  </p>
                </div>
              </div>

              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <p className="text-xs uppercase tracking-wider mb-3" style={{ color: 'var(--text-muted)' }}>Readiness flags</p>
                <div className="flex flex-wrap gap-2">
                  {[
                    ['session', selectedEntry.account?.readiness.session_ready],
                    ['balance', selectedEntry.account?.readiness.balance_ready],
                    ['dry-run', selectedEntry.account?.readiness.dry_run_ready],
                    ['placement', selectedEntry.account?.readiness.placement_ready],
                    ['approval', selectedEntry.account?.readiness.approval_required],
                    ['real money', selectedEntry.account?.readiness.real_money_enabled],
                  ].map(([label, value]) => (
                    <span key={String(label)} className={`badge ${value ? 'badge-success' : 'badge-warning'}`}>{label}: {value ? 'yes' : 'no'}</span>
                  ))}
                </div>
                {(selectedEntry.account?.control_issues.length || selectedEntry.account?.readiness.blocking_reasons.length) ? (
                  <div className="space-y-2 mt-4">
                    {selectedEntry.account?.control_issues.map((issue) => (
                      <p key={issue} className="text-xs" style={{ color: 'var(--accent-red)' }}>- {issue}</p>
                    ))}
                    {selectedEntry.account?.readiness.blocking_reasons.map((reason) => (
                      <p key={reason} className="text-xs" style={{ color: 'var(--text-secondary)' }}>- {reason}</p>
                    ))}
                  </div>
                ) : null}
              </div>
            </div>
          ) : (
            <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
              <BadgeAlert size={24} className="mx-auto mb-3 opacity-40" />
              <p className="text-sm">Выберите букмекера из heatmap или risk list для детального drill-down.</p>
            </div>
          )}
        </motion.div>
      </div>
    </motion.div>
  )
}
