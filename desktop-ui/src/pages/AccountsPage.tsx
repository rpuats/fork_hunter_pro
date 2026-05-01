import { useEffect, useMemo, useState } from 'react'
import { motion } from 'framer-motion'
import { openUrl } from '@tauri-apps/plugin-opener'
import { AlertTriangle, ArrowDownLeft, ArrowUpRight, BadgeAlert, CircleDollarSign, Landmark, PlusCircle, Power, RefreshCw, ShieldAlert, ShieldCheck, ShieldQuestion, User, Wallet, X } from 'lucide-react'
import type {
  AccountControlUpdate,
  AccountSessionImportPayload,
  AccountSessionSummary,
  AccountStateResponse,
  BankrollRecommendationsResponse,
  BankrollState,
  DepositAllocationTarget,
  ExecutionStateAudit,
} from '../types'

interface AccountsPageProps {
  accounts: AccountStateResponse[]
  accountsSummary: AccountSessionSummary | null
  bankrollState: BankrollState | null
  bankrollRecommendations: BankrollRecommendationsResponse | null
  executionState: ExecutionStateAudit | null
  focusedBookmaker: string | null
  onBootstrapAccountSession: (bookmaker: string, login?: string, sessionHint?: string, importPayload?: AccountSessionImportPayload) => Promise<AccountStateResponse | null>
  onRefreshAccountBalance: (bookmaker: string) => Promise<AccountStateResponse | null>
  onUpdateAccountControl: (bookmaker: string, update: AccountControlUpdate) => Promise<AccountStateResponse | null>
}

type NormalizedUrgency = 'high' | 'medium' | 'low'

const BOOKMAKER_LOGIN_URLS: Record<string, string> = {
  pari: 'https://pari.ru/',
  fonbet: 'https://www.fon.bet/',
  marathon: 'https://www.marathonbet.ru/',
  zenit: 'https://zenit.win/',
  betcity: 'https://betcity.ru/',
  baltbet: 'https://www.baltbet.ru/',
  bettery: 'https://bettery.ru/',
  leon: 'https://leon.ru/',
  sportbet: 'https://sportbet.ru/',
  bet24: 'https://24betting.ru/',
}

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

function formatRelativeAge(value: string | null | undefined) {
  if (!value) return 'missing'

  const timestamp = new Date(value).getTime()
  if (Number.isNaN(timestamp)) return 'unknown'

  const deltaMinutes = Math.max(Math.round((Date.now() - timestamp) / 60000), 0)
  if (deltaMinutes < 1) return '<1m'
  if (deltaMinutes < 60) return `${deltaMinutes}m`

  const hours = Math.floor(deltaMinutes / 60)
  const minutes = deltaMinutes % 60
  return minutes > 0 ? `${hours}h ${minutes}m` : `${hours}h`
}

function freshnessLevel(value: string | null | undefined, warningMinutes: number, staleMinutes: number) {
  if (!value) return 'missing' as const

  const timestamp = new Date(value).getTime()
  if (Number.isNaN(timestamp)) return 'unknown' as const

  const deltaMinutes = Math.max((Date.now() - timestamp) / 60000, 0)
  if (deltaMinutes >= staleMinutes) return 'stale' as const
  if (deltaMinutes >= warningMinutes) return 'warning' as const
  return 'fresh' as const
}

function freshnessBadgeClass(level: ReturnType<typeof freshnessLevel>) {
  switch (level) {
    case 'fresh':
      return 'badge-success'
    case 'warning':
      return 'badge-warning'
    default:
      return 'badge-danger'
  }
}

function formatExecutionMode(mode: string | null | undefined) {
  if (!mode) return 'No account'
  return mode.replace(/([a-z])([A-Z])/g, '$1 $2')
}

function isSemiAutoExecutionMode(mode: string | null | undefined) {
  const normalized = String(mode ?? '').toLowerCase()
  return normalized.includes('semi') || normalized.includes('armed') || normalized === 'real'
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

function bookmakerLoginUrl(bookmaker: string) {
  const normalized = bookmaker.toLowerCase()
  return BOOKMAKER_LOGIN_URLS[normalized] ?? `https://www.google.com/search?q=${encodeURIComponent(`${bookmaker} login`)}`
}

export function AccountsPage({ accounts, accountsSummary, bankrollState, bankrollRecommendations, executionState, focusedBookmaker, onBootstrapAccountSession, onRefreshAccountBalance, onUpdateAccountControl }: AccountsPageProps) {
  const [selectedBookmaker, setSelectedBookmaker] = useState<string | null>(null)
  const [testBookmaker, setTestBookmaker] = useState('pari')
  const [accountLogin, setAccountLogin] = useState('')
  const [sessionHint, setSessionHint] = useState('')
  const [rawSessionImport, setRawSessionImport] = useState('')
  const [cookieHeader, setCookieHeader] = useState('')
  const [authorizationHeader, setAuthorizationHeader] = useState('')
  const [csrfToken, setCsrfToken] = useState('')
  const [userAgent, setUserAgent] = useState('')
  const [availableBalance, setAvailableBalance] = useState('10000')
  const [expiresInHours, setExpiresInHours] = useState('8')
  const [accountModalOpen, setAccountModalOpen] = useState(false)
  const [loginPageOpened, setLoginPageOpened] = useState(false)
  const [bootstrappingSession, setBootstrappingSession] = useState(false)
  const [accountAction, setAccountAction] = useState<string | null>(null)
  const [bulkAuthAccounts, setBulkAuthAccounts] = useState<Record<string, { login: string; password: string }>>({})
  const [bulkAuthInProgress, setBulkAuthInProgress] = useState(false)
  const [bulkAuthResults, setBulkAuthResults] = useState<{ bookmaker: string; status: string }[]>([])
  const [bulkAuthModalOpen, setBulkAuthModalOpen] = useState(false)
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
  const executionReadiness = executionState?.readiness ?? {
    total_bookmakers: summary.total_bookmakers,
    accounts_configured: summary.accounts_configured,
    accounts_enabled: summary.accounts_enabled,
    auth_ready: accounts.filter((entry) => entry.readiness.session_ready).length,
    sessions_authenticated: summary.sessions_authenticated,
    balances_cached: summary.balances_cached,
    dry_run_ready: summary.ready_for_dry_run,
    placement_ready: summary.ready_for_execution,
    approval_required: accounts.filter((entry) => entry.readiness.approval_required).length,
    submit_blocked_by_safe_mode: accounts.filter((entry) => entry.readiness.submit_blocked_by_safe_mode).length,
    operator_attention_required: accounts.filter((entry) => entry.readiness.approval_required || Boolean(entry.readiness.operator_action)).length,
  }
  const authGap = Math.max(executionReadiness.auth_ready - executionReadiness.sessions_authenticated, 0)
  const executionReadinessMap = useMemo(
    () => new Map((executionState?.bookmaker_readiness ?? []).map((entry) => [entry.bookmaker.toLowerCase(), entry])),
    [executionState],
  )
  const authFocusRows = (executionState?.bookmaker_readiness ?? accounts.map((entry) => ({
    bookmaker: entry.bookmaker,
    account_configured: Boolean(entry.account),
    account_enabled: Boolean(entry.account?.enabled),
    execution_mode: entry.account?.mode ?? null,
    requires_session: entry.capability.requires_session,
    auth_ready: entry.readiness.session_ready,
    session_authenticated: entry.readiness.session_ready,
    balance_cached: Boolean(entry.balance),
    dry_run_ready: entry.readiness.dry_run_ready,
    placement_ready: entry.readiness.placement_ready,
    approval_required: entry.readiness.approval_required,
    submit_blocked_by_safe_mode: entry.readiness.submit_blocked_by_safe_mode,
    operator_action: entry.readiness.operator_action,
    blocking_reasons: entry.readiness.blocking_reasons,
  })))
    .map((entry) => ({
      ...entry,
      priority:
        (entry.submit_blocked_by_safe_mode ? 80 : 0)
        + (entry.approval_required ? 45 : 0)
        + (entry.requires_session && !entry.auth_ready ? 30 : 0)
        + (entry.requires_session && !entry.session_authenticated ? 25 : 0)
        + (!entry.balance_cached ? 16 : 0)
        + (!entry.placement_ready ? 22 : 0),
    }))
    .filter((entry) => entry.priority > 0)
    .sort((a, b) => b.priority - a.priority || a.bookmaker.localeCompare(b.bookmaker))
    .slice(0, 4)

  const semiAutoRows = useMemo(() => {
    return accounts
      .map((entry) => {
        const executionEntry = executionReadinessMap.get(entry.bookmaker.toLowerCase())
        const executionMode = executionEntry?.execution_mode ?? entry.account?.mode ?? null
        const sessionLevel = freshnessLevel(entry.session?.last_synced_at, 15, 30)
        const balanceLevel = freshnessLevel(entry.balance?.captured_at, 5, 15)
        const staleSignals = [sessionLevel, balanceLevel].filter((level) => level === 'stale' || level === 'missing' || level === 'unknown').length
        const needsSession = entry.capability.requires_session
        const semiAutoEnabled = Boolean(entry.account?.enabled && entry.capability.supports_bet_placement)
        const semiAutoMode = isSemiAutoExecutionMode(executionMode)
        const working = semiAutoEnabled
          && semiAutoMode
          && entry.readiness.placement_ready
          && !entry.readiness.submit_blocked_by_safe_mode
          && (!needsSession || entry.readiness.session_ready)
          && balanceLevel === 'fresh'
          && (sessionLevel === 'fresh' || !needsSession)

        let operatorStep = 'Working semi-auto path: monitor only'
        if (entry.readiness.submit_blocked_by_safe_mode) {
          operatorStep = 'Safe mode blocks submit'
        } else if (entry.readiness.approval_required) {
          operatorStep = 'Approval required before semi-auto submit'
        } else if (needsSession && (!entry.readiness.session_ready || sessionLevel !== 'fresh')) {
          operatorStep = 'Refresh auth/session before next handoff'
        } else if (!entry.readiness.balance_ready || balanceLevel !== 'fresh') {
          operatorStep = 'Refresh balance snapshot before semi-auto'
        } else if (!semiAutoMode) {
          operatorStep = 'Execution mode not in semi-auto lane'
        } else if (!entry.readiness.placement_ready) {
          operatorStep = entry.readiness.operator_action ?? entry.readiness.blocking_reasons[0] ?? 'Placement path still blocked'
        }

        const priority =
          (working ? 120 : 0)
          + (semiAutoMode ? 40 : 0)
          + (entry.readiness.placement_ready ? 30 : 0)
          - (entry.readiness.submit_blocked_by_safe_mode ? 80 : 0)
          - (entry.readiness.approval_required ? 25 : 0)
          - (needsSession && !entry.readiness.session_ready ? 35 : 0)
          - ((balanceLevel === 'stale' || balanceLevel === 'missing' || balanceLevel === 'unknown') ? 30 : balanceLevel === 'warning' ? 10 : 0)
          - ((sessionLevel === 'stale' || sessionLevel === 'missing' || sessionLevel === 'unknown') ? 25 : sessionLevel === 'warning' ? 8 : 0)

        return {
          bookmaker: entry.bookmaker,
          executionMode,
          semiAutoMode,
          semiAutoEnabled,
          working,
          sessionLevel,
          sessionAge: formatRelativeAge(entry.session?.last_synced_at),
          balanceLevel,
          balanceAge: formatRelativeAge(entry.balance?.captured_at),
          staleSignals,
          requiresSession: needsSession,
          placementReady: entry.readiness.placement_ready,
          approvalRequired: entry.readiness.approval_required,
          safeModeBlocked: entry.readiness.submit_blocked_by_safe_mode,
          operatorStep,
          note: entry.readiness.operator_action ?? entry.readiness.blocking_reasons[0] ?? null,
          priority,
        }
      })
      .filter((entry) => entry.semiAutoEnabled || entry.working || entry.semiAutoMode)
      .sort((a, b) => b.priority - a.priority || a.staleSignals - b.staleSignals || a.bookmaker.localeCompare(b.bookmaker))
      .slice(0, 6)
  }, [accounts, executionReadinessMap])

  const semiAutoSummary = useMemo(() => ({
    working: semiAutoRows.filter((entry) => entry.working).length,
    stale: semiAutoRows.filter((entry) => entry.staleSignals > 0).length,
    gated: semiAutoRows.filter((entry) => entry.safeModeBlocked || entry.approvalRequired || !entry.placementReady).length,
  }), [semiAutoRows])

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

  const accountBookmakers = useMemo(() => {
    const preferred = ['pari', 'fonbet', 'marathon', 'zenit', 'betcity', 'baltbet', 'bettery']
    const fromAccounts = accounts.map((entry) => entry.bookmaker.toLowerCase())
    return Array.from(new Set([...preferred, ...fromAccounts])).sort()
  }, [accounts])

  const selectedLoginUrl = bookmakerLoginUrl(testBookmaker)

  const handleOpenLoginPage = async () => {
    setLoginPageOpened(true)
    try {
      await openUrl(selectedLoginUrl)
    } catch {
      const opened = window.open(selectedLoginUrl, '_blank', 'noopener,noreferrer')
      if (!opened) {
        window.location.assign(selectedLoginUrl)
      }
    }
  }

  const handleCreateTestAccount = async () => {
    if (!accountLogin.trim()) return

    const parsedBalance = Number(availableBalance.replace(',', '.'))
    const parsedExpires = Number(expiresInHours)
    const importPayload: AccountSessionImportPayload = {
      rawImport: rawSessionImport.trim() || undefined,
      cookieHeader: cookieHeader.trim() || undefined,
      authorizationHeader: authorizationHeader.trim() || undefined,
      csrfToken: csrfToken.trim() || undefined,
      userAgent: userAgent.trim() || undefined,
      availableBalance: Number.isFinite(parsedBalance) && parsedBalance >= 0 ? parsedBalance : 10000,
      expiresInHours: Number.isFinite(parsedExpires) && parsedExpires > 0 ? parsedExpires : 8,
    }

    setBootstrappingSession(true)
    try {
      const account = await onBootstrapAccountSession(testBookmaker, accountLogin.trim(), sessionHint.trim() || undefined, importPayload)
      if (account) {
        setSelectedBookmaker(account.bookmaker)
        setAccountModalOpen(false)
        setSessionHint('')
        setRawSessionImport('')
        setCookieHeader('')
        setAuthorizationHeader('')
        setCsrfToken('')
        setUserAgent('')
      }
    } finally {
      setBootstrappingSession(false)
    }
  }

  const runAccountAction = async (bookmaker: string, action: string, handler: () => Promise<AccountStateResponse | null>) => {
    const actionKey = `${bookmaker.toLowerCase()}:${action}`
    setAccountAction(actionKey)
    try {
      const account = await handler()
      if (account) setSelectedBookmaker(account.bookmaker)
    } finally {
      setAccountAction(null)
    }
  }

  const isAccountActionRunning = (bookmaker: string, action: string) => accountAction === `${bookmaker.toLowerCase()}:${action}`

  // Bulk authorization function
  const handleBulkAuthorize = async () => {
    const entries = Object.entries(bulkAuthAccounts).filter(([_, creds]) => creds.login && creds.password)
    if (entries.length === 0) {
      alert('Введите логин и пароль хотя бы для одной БК')
      return
    }

    setBulkAuthInProgress(true)
    setBulkAuthResults([])

    try {
      const response = await fetch('http://localhost:8080/api/v2/accounts/bulk-automated-login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          accounts: entries.map(([bookmaker, creds]) => ({
            bookmaker,
            login: creds.login,
            password: creds.password,
          })),
          wait_timeout_secs: 240,
          auto_close_after_auth: true,
        }),
      })

      const data = await response.json()
      
      if (data.success && data.data) {
        setBulkAuthResults(data.data.results.map((r: any) => ({
          bookmaker: r.bookmaker,
          status: r.authenticated ? '✅ Успешно' : `❌ ${r.status}`,
        })))
        
        // Refresh accounts after successful auth
        setTimeout(() => {
          window.location.reload()
        }, 3000)
      } else {
        alert(`Ошибка: ${data.error || 'Неизвестная ошибка'}`)
      }
    } catch (error) {
      alert(`Ошибка запроса: ${error}`)
    } finally {
      setBulkAuthInProgress(false)
    }
  }

  const updateBulkAuthCredential = (bookmaker: string, field: 'login' | 'password', value: string) => {
    setBulkAuthAccounts(prev => ({
      ...prev,
      [bookmaker]: {
        ...prev[bookmaker],
        [field]: value,
      },
    }))
  }

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="space-y-6">
      <motion.div variants={item} className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <h2 className="text-2xl font-bold">Accounts / Bankroll Readiness</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Read-only экран поверх `/api/v1/accounts`, `/api/v1/accounts/summary`, `/api/v1/bankroll` и `/api/v1/bankroll/recommendations`.
          </p>
        </div>

        <div className="flex flex-wrap items-center gap-3 text-xs">
          <div className="flex items-center gap-2 rounded-xl px-3 py-2" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            <button
              type="button"
              onClick={() => setAccountModalOpen(true)}
              className="inline-flex items-center gap-1 rounded-lg px-2 py-1 font-medium transition-opacity disabled:opacity-50"
              style={{ background: 'rgba(63, 185, 80, 0.14)', color: 'var(--accent-green)' }}
            >
              <PlusCircle size={14} />
              Добавить аккаунт
            </button>
          </div>
          <div className="rounded-xl px-3 py-2" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            Accounts snapshot {accounts.length}
          </div>
          <div className="rounded-xl px-3 py-2" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            Bankroll update {formatDateTime(bankrollState?.updated_at ?? null)}
          </div>
          
          {/* Bulk Authorization Button */}
          <div className="flex items-center gap-2 rounded-xl px-3 py-2" style={{ background: 'rgba(47, 129, 247, 0.14)', border: '1px solid rgba(47, 129, 247, 0.3)' }}>
            <button
              type="button"
              onClick={() => setBulkAuthModalOpen(true)}
              disabled={bulkAuthInProgress}
              className="inline-flex items-center gap-1 rounded-lg px-2 py-1 font-medium transition-opacity disabled:opacity-50"
              style={{ color: 'var(--accent-blue, #2f81f7)' }}
            >
              {bulkAuthInProgress ? <RefreshCw size={14} className="animate-spin" /> : <ShieldCheck size={14} />}
              {bulkAuthInProgress ? 'Авторизация...' : 'Авторизовать все'}
            </button>
          </div>
        </div>
      </motion.div>

      {accountModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4" style={{ background: 'rgba(1, 4, 9, 0.72)' }}>
          <div className="w-full max-w-2xl max-h-[92vh] overflow-auto rounded-2xl p-5" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            <div className="flex items-start justify-between gap-4 mb-4">
              <div>
                <h3 className="text-lg font-semibold">Добавить аккаунт БК</h3>
                <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                  Safe-mode bootstrap после ручного входа: пароль в Fork Hunter вводить не нужно.
                </p>
              </div>
              <button
                type="button"
                onClick={() => setAccountModalOpen(false)}
                className="rounded-lg p-2 transition-colors"
                style={{ color: 'var(--text-muted)' }}
              >
                <X size={18} />
              </button>
            </div>

            <div className="space-y-3">
              <label className="block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                Букмекер
                <select
                  value={testBookmaker}
                  onChange={(event) => {
                    setTestBookmaker(event.target.value)
                    setLoginPageOpened(false)
                  }}
                  className="mt-1 w-full rounded-xl px-3 py-2 outline-none"
                  style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                >
                  {accountBookmakers.map((bookmaker) => (
                    <option key={bookmaker} value={bookmaker}>{bookmaker}</option>
                  ))}
                </select>
              </label>

              <div className="rounded-xl p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                  <div>
                    <p className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>Шаг 1: вход на сайт БК</p>
                    <a
                      href={selectedLoginUrl}
                      className="text-xs mt-1 block break-all underline decoration-dotted"
                      style={{ color: 'var(--text-muted)' }}
                    >
                      {selectedLoginUrl}
                    </a>
                  </div>
                  <button
                    type="button"
                    onClick={handleOpenLoginPage}
                    className="rounded-xl px-3 py-2 text-xs font-semibold"
                    style={{ background: 'rgba(88, 166, 255, 0.14)', color: 'var(--accent-blue)' }}
                  >
                    Открыть вход
                  </button>
                </div>
                <p className="text-xs mt-3" style={{ color: loginPageOpened ? 'var(--accent-green)' : 'var(--text-muted)' }}>
                  {loginPageOpened ? 'Если новое окно не открылось, приложение перейдёт на сайт БК в этом же окне. Назад можно вернуться кнопкой Back.' : 'Сначала открой сайт БК и авторизуйся вручную. Автозахват cookies ещё не включён.'}
                </p>
              </div>

              <label className="block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                Шаг 2: логин для safe-mode label
                <input
                  value={accountLogin}
                  onChange={(event) => setAccountLogin(event.target.value)}
                  placeholder="phone / email / account id"
                  className="mt-1 w-full rounded-xl px-3 py-2 outline-none"
                  style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                />
              </label>

              <label className="block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                Заметка к ручной сессии (опционально)
                <input
                  value={sessionHint}
                  onChange={(event) => setSessionHint(event.target.value)}
                  placeholder="например: logged-in browser / 2FA ok"
                  className="mt-1 w-full rounded-xl px-3 py-2 outline-none"
                  style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                />
              </label>

              <div className="rounded-xl p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <p className="text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>Шаг 3: импорт реальной browser-сессии</p>
                <p className="text-xs mt-1 leading-5" style={{ color: 'var(--text-muted)' }}>
                  После входа на сайт БК скопируй из DevTools один authenticated запрос как cURL или вставь Cookie header. Raw cookies хранятся только в runtime памяти backend и не уходят в response/SQLite.
                </p>
                <textarea
                  value={rawSessionImport}
                  onChange={(event) => setRawSessionImport(event.target.value)}
                  placeholder="curl 'https://...' -H 'Cookie: ...' -H 'User-Agent: ...'"
                  rows={4}
                  className="mt-3 w-full rounded-xl px-3 py-2 text-xs outline-none"
                  style={{ background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                />
                <div className="grid grid-cols-1 gap-2 mt-3 sm:grid-cols-2">
                  <input
                    value={cookieHeader}
                    onChange={(event) => setCookieHeader(event.target.value)}
                    placeholder="Cookie: sid=... (optional)"
                    className="rounded-xl px-3 py-2 text-xs outline-none"
                    style={{ background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                  />
                  <input
                    value={authorizationHeader}
                    onChange={(event) => setAuthorizationHeader(event.target.value)}
                    placeholder="Authorization: Bearer ..."
                    className="rounded-xl px-3 py-2 text-xs outline-none"
                    style={{ background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                  />
                  <input
                    value={csrfToken}
                    onChange={(event) => setCsrfToken(event.target.value)}
                    placeholder="X-CSRF token"
                    className="rounded-xl px-3 py-2 text-xs outline-none"
                    style={{ background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                  />
                  <input
                    value={userAgent}
                    onChange={(event) => setUserAgent(event.target.value)}
                    placeholder="User-Agent"
                    className="rounded-xl px-3 py-2 text-xs outline-none"
                    style={{ background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <label className="block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                  Доступный баланс
                  <input
                    value={availableBalance}
                    onChange={(event) => setAvailableBalance(event.target.value)}
                    inputMode="decimal"
                    className="mt-1 w-full rounded-xl px-3 py-2 outline-none"
                    style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                  />
                </label>
                <label className="block text-xs font-medium" style={{ color: 'var(--text-secondary)' }}>
                  TTL сессии, часов
                  <input
                    value={expiresInHours}
                    onChange={(event) => setExpiresInHours(event.target.value)}
                    inputMode="numeric"
                    className="mt-1 w-full rounded-xl px-3 py-2 outline-none"
                    style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                  />
                </label>
              </div>

              <div className="rounded-xl p-3 text-xs" style={{ background: 'rgba(210, 153, 34, 0.12)', color: 'var(--text-secondary)', border: '1px solid rgba(210, 153, 34, 0.22)' }}>
                Не вводи сюда пароль. Для реальной авторизации нужен Cookie или Authorization header из уже залогиненного браузера; backend вернёт только redacted summary.
              </div>
            </div>

            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setAccountModalOpen(false)}
                className="rounded-xl px-4 py-2 text-sm"
                style={{ background: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}
              >
                Отмена
              </button>
              <button
                type="button"
                onClick={handleCreateTestAccount}
                disabled={bootstrappingSession || !accountLogin.trim()}
                className="rounded-xl px-4 py-2 text-sm font-semibold transition-opacity disabled:opacity-50"
                style={{ background: 'rgba(63, 185, 80, 0.18)', color: 'var(--accent-green)' }}
              >
                {bootstrappingSession ? 'Сохраняю...' : 'Сохранить safe-mode сессию'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Bulk Authorization Modal */}
      {bulkAuthModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4" style={{ background: 'rgba(1, 4, 9, 0.72)' }}>
          <div className="w-full max-w-4xl max-h-[92vh] overflow-auto rounded-2xl p-5" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            <div className="flex items-start justify-between gap-4 mb-4">
              <div>
                <h3 className="text-lg font-semibold">🔐 Массовая авторизация БК</h3>
                <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                  Введите логин и пароль для нескольких БК. Система последовательно откроет браузеры, введёт данные и дождётся captcha/2FA.
                </p>
              </div>
              <button
                type="button"
                onClick={() => setBulkAuthModalOpen(false)}
                className="rounded-lg p-2 transition-colors"
                style={{ color: 'var(--text-muted)' }}
              >
                <X size={18} />
              </button>
            </div>

            <div className="space-y-4">
              {/* List of bookmakers to authorize */}
              <div className="grid gap-3 md:grid-cols-2">
                {['pari', 'fonbet', 'marathon', 'bettery', 'leon'].map((bookmaker) => (
                  <div
                    key={bookmaker}
                    className="rounded-xl p-3"
                    style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}
                  >
                    <div className="flex items-center gap-2 mb-3">
                      <span className="text-sm font-medium capitalize">{bookmaker}</span>
                      <span className="text-xs px-2 py-0.5 rounded-full" style={{ 
                        background: bulkAuthAccounts[bookmaker]?.login ? 'rgba(63, 185, 80, 0.2)' : 'rgba(139, 148, 158, 0.2)',
                        color: bulkAuthAccounts[bookmaker]?.login ? 'var(--accent-green)' : 'var(--text-muted)'
                      }}>
                        {bulkAuthAccounts[bookmaker]?.login ? '✓ Готов' : '—'}
                      </span>
                    </div>
                    
                    <div className="space-y-2">
                      <input
                        value={bulkAuthAccounts[bookmaker]?.login || ''}
                        onChange={(e) => updateBulkAuthCredential(bookmaker, 'login', e.target.value)}
                        placeholder="Логин (телефон/email)"
                        className="w-full rounded-lg px-3 py-2 text-sm outline-none"
                        style={{ background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                      />
                      <input
                        type="password"
                        value={bulkAuthAccounts[bookmaker]?.password || ''}
                        onChange={(e) => updateBulkAuthCredential(bookmaker, 'password', e.target.value)}
                        placeholder="Пароль"
                        className="w-full rounded-lg px-3 py-2 text-sm outline-none"
                        style={{ background: 'var(--var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
                      />
                    </div>
                  </div>
                ))}
              </div>

              {/* Info box */}
              <div className="rounded-xl p-3 text-xs" style={{ background: 'rgba(47, 129, 247, 0.12)', color: 'var(--text-secondary)', border: '1px solid rgba(47, 129, 247, 0.22)' }}>
                <p className="font-medium mb-1">📋 Как это работает:</p>
                <ul className="space-y-1 ml-4 list-disc">
                  <li>Система откроет браузер для каждого БК по очереди</li>
                  <li>Для телефонов автоматически добавится +7 если нужно</li>
                  <li>После ввода логина/пароля система ждёт captcha/2FA (до 4 минут)</li>
                  <li>После успешного входа браузер закрывается, сохраняются cookies</li>
                  <li>Баланс автоматически определяется со страницы</li>
                </ul>
              </div>

              {/* Results */}
              {bulkAuthResults.length > 0 && (
                <div className="rounded-xl p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                  <p className="text-sm font-medium mb-2">Результаты:</p>
                  <div className="space-y-1">
                    {bulkAuthResults.map((result, idx) => (
                      <div key={idx} className="flex items-center justify-between text-sm">
                        <span className="capitalize">{result.bookmaker}</span>
                        <span style={{ 
                          color: result.status.includes('✅') ? 'var(--accent-green)' : 'var(--accent-red)'
                        }}>
                          {result.status}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setBulkAuthModalOpen(false)}
                className="rounded-xl px-4 py-2 text-sm"
                style={{ background: 'var(--bg-secondary)', color: 'var(--text-secondary)' }}
              >
                Отмена
              </button>
              <button
                type="button"
                onClick={handleBulkAuthorize}
                disabled={bulkAuthInProgress}
                className="rounded-xl px-4 py-2 text-sm font-semibold transition-opacity disabled:opacity-50"
                style={{ background: 'rgba(47, 129, 247, 0.18)', color: 'var(--accent-blue)' }}
              >
                {bulkAuthInProgress ? (
                  <span className="flex items-center gap-2">
                    <RefreshCw size={14} className="animate-spin" />
                    Авторизация...
                  </span>
                ) : (
                  <span className="flex items-center gap-2">
                    <ShieldCheck size={14} />
                    Авторизовать все
                  </span>
                )}
              </button>
            </div>
          </div>
        </div>
      )}

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

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between mb-4">
          <div>
            <h3 className="text-base font-semibold">Execution auth surface</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              Read-only auth/readiness lens from `/api/v1/execution/state` aligned with `/api/v1/accounts`.
            </p>
          </div>
          <div className="flex flex-wrap gap-2 text-xs">
            <span className={`badge ${authGap > 0 ? 'badge-warning' : 'badge-success'}`}>auth drift {authGap}</span>
            <span className={`badge ${executionReadiness.submit_blocked_by_safe_mode > 0 ? 'badge-danger' : 'badge-success'}`}>safe mode {executionReadiness.submit_blocked_by_safe_mode}</span>
            <span className={`badge ${executionReadiness.operator_attention_required > 0 ? 'badge-warning' : 'badge-success'}`}>attention {executionReadiness.operator_attention_required}</span>
          </div>
        </div>

        <div className="grid grid-cols-2 xl:grid-cols-5 gap-3 mb-4">
          {[
            ['Auth ready', executionReadiness.auth_ready, 'session-ready in readiness checks'],
            ['Authenticated', executionReadiness.sessions_authenticated, 'active sessions in execution snapshot'],
            ['Balances', executionReadiness.balances_cached, 'cached balance snapshots'],
            ['Placement ready', executionReadiness.placement_ready, 'books available to execution'],
            ['Approval / safe', `${executionReadiness.approval_required} / ${executionReadiness.submit_blocked_by_safe_mode}`, 'manual gates on submit path'],
          ].map(([label, value, detail]) => (
            <div key={String(label)} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{label}</p>
              <p className="text-2xl font-semibold mt-2">{value}</p>
              <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>{detail}</p>
            </div>
          ))}
        </div>

        {authFocusRows.length > 0 ? (
          <div className="grid gap-3 xl:grid-cols-2">
            {authFocusRows.map((entry) => (
              <button
                key={entry.bookmaker}
                type="button"
                onClick={() => setSelectedBookmaker(entry.bookmaker)}
                className="w-full rounded-xl p-4 text-left transition-colors"
                style={{ background: 'var(--bg-secondary)', border: `1px solid ${entry.submit_blocked_by_safe_mode ? 'rgba(248, 81, 73, 0.22)' : entry.approval_required || !entry.placement_ready ? 'rgba(210, 153, 34, 0.22)' : 'var(--border-color)'}` }}
              >
                <div className="flex items-start justify-between gap-3 mb-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="text-sm font-semibold">{entry.bookmaker}</p>
                    <span className={`badge ${entry.requires_session && !entry.session_authenticated ? 'badge-warning' : 'badge-success'}`}>{entry.requires_session ? entry.session_authenticated ? 'session active' : 'session missing' : 'no session req'}</span>
                    <span className={`badge ${entry.placement_ready ? 'badge-success' : 'badge-warning'}`}>{entry.placement_ready ? 'placement ready' : 'placement blocked'}</span>
                  </div>
                  <span className={`badge ${entry.priority >= 80 ? 'badge-danger' : 'badge-warning'}`}>priority {entry.priority}</span>
                </div>
                <p className="text-xs" style={{ color: 'var(--text-secondary)' }}>
                  {entry.operator_action ?? entry.blocking_reasons[0] ?? (entry.balance_cached ? 'No explicit operator action in execution snapshot.' : 'Balance snapshot missing from execution path.')}
                </p>
              </button>
            ))}
          </div>
        ) : (
          <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
            <ShieldCheck size={24} className="mx-auto mb-3 opacity-40" />
            <p className="text-sm">Execution auth surface не показал дополнительных blockers поверх accounts snapshot.</p>
          </div>
        )}
      </motion.div>

      <motion.div variants={item} className="glass-card p-5">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between mb-4">
          <div>
            <h3 className="text-base font-semibold">Semi-auto working board</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
              Extra read-only step after auth/readiness/stale signals: shows which books are actually usable for semi-auto right now.
            </p>
          </div>
          <div className="flex flex-wrap gap-2 text-xs">
            <span className={`badge ${semiAutoSummary.working > 0 ? 'badge-success' : 'badge-warning'}`}>working {semiAutoSummary.working}</span>
            <span className={`badge ${semiAutoSummary.stale > 0 ? 'badge-warning' : 'badge-success'}`}>stale watch {semiAutoSummary.stale}</span>
            <span className={`badge ${semiAutoSummary.gated > 0 ? 'badge-warning' : 'badge-success'}`}>gated {semiAutoSummary.gated}</span>
          </div>
        </div>

        <div className="grid grid-cols-2 xl:grid-cols-4 gap-3 mb-4">
          {[
            ['Working now', semiAutoSummary.working.toString(), 'fresh auth + fresh balance + placement path'],
            ['Semi-auto tracked', semiAutoRows.length.toString(), 'enabled books inside semi-auto lane'],
            ['Stale signals', semiAutoSummary.stale.toString(), 'session or balance freshness degraded'],
            ['Operator gates', semiAutoSummary.gated.toString(), 'approval, safe mode or blocked placement'],
          ].map(([label, value, detail]) => (
            <div key={String(label)} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>{label}</p>
              <p className="text-2xl font-semibold mt-2">{value}</p>
              <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>{detail}</p>
            </div>
          ))}
        </div>

        {semiAutoRows.length > 0 ? (
          <div className="grid gap-3 xl:grid-cols-2">
            {semiAutoRows.map((entry) => (
              <button
                key={entry.bookmaker}
                type="button"
                onClick={() => setSelectedBookmaker(entry.bookmaker)}
                className="w-full rounded-xl p-4 text-left transition-colors"
                style={{
                  background: selectedEntry?.bookmaker === entry.bookmaker ? 'var(--bg-hover)' : 'var(--bg-secondary)',
                  border: `1px solid ${entry.working ? 'rgba(63, 185, 80, 0.22)' : entry.safeModeBlocked ? 'rgba(248, 81, 73, 0.22)' : 'rgba(210, 153, 34, 0.22)'}`,
                }}
              >
                <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                  <div>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-semibold">{entry.bookmaker}</p>
                      <span className={`badge ${entry.working ? 'badge-success' : 'badge-warning'}`}>{entry.working ? 'working now' : 'watch / gated'}</span>
                      <span className={`badge ${entry.semiAutoMode ? 'badge-info' : 'badge-warning'}`}>{formatExecutionMode(entry.executionMode)}</span>
                    </div>

                    <div className="flex flex-wrap gap-2 mt-2">
                      <span className={`badge ${entry.requiresSession ? freshnessBadgeClass(entry.sessionLevel) : 'badge-info'}`}>auth {entry.requiresSession ? entry.sessionAge : 'n/a'}</span>
                      <span className={`badge ${freshnessBadgeClass(entry.balanceLevel)}`}>balance {entry.balanceAge}</span>
                      <span className={`badge ${entry.placementReady ? 'badge-success' : 'badge-warning'}`}>placement {entry.placementReady ? 'ready' : 'blocked'}</span>
                      {entry.approvalRequired ? <span className="badge badge-warning">approval</span> : null}
                      {entry.safeModeBlocked ? <span className="badge badge-danger">safe mode</span> : null}
                    </div>

                    <p className="text-sm mt-3 leading-6">{entry.operatorStep}</p>
                    <p className="text-xs mt-2" style={{ color: 'var(--text-secondary)' }}>
                      {entry.note ?? (entry.working ? 'Operator can keep this book in the semi-auto working pool.' : 'Freshness and readiness need review before the next semi-auto cycle.')}
                    </p>
                  </div>

                  <div className="text-left lg:text-right shrink-0">
                    <p className="text-xs" style={{ color: 'var(--text-muted)' }}>stale signals</p>
                    <p className={`text-sm font-semibold ${entry.staleSignals > 0 ? 'profit-negative' : 'profit-positive'}`}>{entry.staleSignals}</p>
                  </div>

                  {/* Quick authorize button */}
                  <div className="shrink-0">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation()
                        updateBulkAuthCredential(entry.bookmaker.toLowerCase(), 'login', '')
                        updateBulkAuthCredential(entry.bookmaker.toLowerCase(), 'password', '')
                        setBulkAuthModalOpen(true)
                      }}
                      className="inline-flex items-center gap-1 rounded-lg px-2 py-1 text-xs font-medium transition-colors"
                      style={{ 
                        background: entry.requiresSession ? 'rgba(248, 81, 73, 0.14)' : 'rgba(63, 185, 80, 0.14)', 
                        color: entry.requiresSession ? 'var(--accent-red)' : 'var(--accent-green)'
                      }}
                    >
                      <User size={12} />
                      {entry.requiresSession ? 'Авторизовать' : 'Обновить'}
                    </button>
                  </div>
                </div>
              </button>
            ))}
          </div>
        ) : (
          <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
            <ShieldQuestion size={24} className="mx-auto mb-3 opacity-40" />
            <p className="text-sm">Semi-auto working board ждёт enabled execution books из accounts/execution snapshots.</p>
          </div>
        )}
      </motion.div>

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
                      {selectedEntry.account?.session_material ? <span className="badge badge-success">real session imported</span> : null}
                    </div>
                    <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>{readinessLabel(selectedEntry.account)}</p>
                    {selectedEntry.account?.session_material ? (
                      <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>
                        {selectedEntry.account.session_material.redacted_hint} · {selectedEntry.account.session_material.persistence}
                      </p>
                    ) : null}
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

                <div className="mt-4 grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={() => runAccountAction(selectedEntry.bookmaker, 'refresh', () => onRefreshAccountBalance(selectedEntry.bookmaker))}
                    disabled={!selectedEntry.account?.account || isAccountActionRunning(selectedEntry.bookmaker, 'refresh')}
                    className="rounded-xl px-3 py-2 text-xs font-semibold transition-opacity disabled:opacity-50"
                    style={{ background: 'rgba(88, 166, 255, 0.14)', color: 'var(--accent-blue)' }}
                  >
                    <span className="inline-flex items-center gap-1"><RefreshCw size={12} /> {isAccountActionRunning(selectedEntry.bookmaker, 'refresh') ? 'Refreshing...' : 'Refresh balance'}</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => runAccountAction(selectedEntry.bookmaker, 'dry-run', () => onUpdateAccountControl(selectedEntry.bookmaker, { enabled: true, armed: false }))}
                    disabled={!selectedEntry.account?.account || isAccountActionRunning(selectedEntry.bookmaker, 'dry-run')}
                    className="rounded-xl px-3 py-2 text-xs font-semibold transition-opacity disabled:opacity-50"
                    style={{ background: 'rgba(63, 185, 80, 0.14)', color: 'var(--accent-green)' }}
                  >
                    Set dry-run
                  </button>
                  <button
                    type="button"
                    onClick={() => runAccountAction(selectedEntry.bookmaker, 'arm', () => onUpdateAccountControl(selectedEntry.bookmaker, { enabled: true, armed: true }))}
                    disabled={!selectedEntry.account?.account || !selectedEntry.account.readiness.can_arm_safely || isAccountActionRunning(selectedEntry.bookmaker, 'arm')}
                    className="rounded-xl px-3 py-2 text-xs font-semibold transition-opacity disabled:opacity-50"
                    style={{ background: 'rgba(210, 153, 34, 0.14)', color: 'var(--accent-yellow)' }}
                  >
                    Arm safe lane
                  </button>
                  <button
                    type="button"
                    onClick={() => runAccountAction(selectedEntry.bookmaker, 'disable', () => onUpdateAccountControl(selectedEntry.bookmaker, { enabled: false, armed: false }))}
                    disabled={!selectedEntry.account?.account || !selectedEntry.account.account.enabled || isAccountActionRunning(selectedEntry.bookmaker, 'disable')}
                    className="rounded-xl px-3 py-2 text-xs font-semibold transition-opacity disabled:opacity-50"
                    style={{ background: 'rgba(248, 81, 73, 0.14)', color: 'var(--accent-red)' }}
                  >
                    <span className="inline-flex items-center gap-1"><Power size={12} /> Disable</span>
                  </button>
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
