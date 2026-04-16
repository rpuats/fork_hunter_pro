import { useMemo } from 'react'
import { motion } from 'framer-motion'
import { Zap, TrendingUp, Users, Clock, Target, BarChart3, ArrowUpRight, ArrowDownRight, Gem, BadgePercent, ShieldX, Activity, PlayCircle, PauseCircle, AlertTriangle, Wallet } from 'lucide-react'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, BarChart, Bar, Cell } from 'recharts'
import { ParserDeepDive } from '../components/ParserDeepDive'
import type { ScannerMetrics, Surebet, Bookmaker, ValueBet, GenerosityIndex, ParserCoverage, ParserHealth, ExecutionOverview } from '../types'

interface DashboardProps {
  metrics: ScannerMetrics | null
  surebets: Surebet[]
  bookmakers: Bookmaker[]
  valueBets: ValueBet[]
  generosityIndices: GenerosityIndex[]
  executionOverview?: ExecutionOverview | null
  parserCoverage: ParserCoverage[]
  parserHealth: ParserHealth[]
}

const container = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: { staggerChildren: 0.05 }
  }
}

const item = {
  hidden: { opacity: 0, y: 20 },
  show: { opacity: 1, y: 0, transition: { duration: 0.3 } }
}

const chartPalette = ['#58a6ff', '#bc8cff', '#3fb950', '#39d2c0', '#d29922', '#f85149']

export function Dashboard({ metrics, surebets, bookmakers, valueBets, generosityIndices, executionOverview, parserCoverage, parserHealth }: DashboardProps) {
  const realSurebetsFound = metrics?.surebets_found ?? 0
  const realEventsParsed = metrics?.events_parsed ?? 0
  const activeBKs = bookmakers.filter(b => b.status === 'active').length
  const cycleSeconds = metrics ? (metrics.cycle_time_ms / 1000).toFixed(1) : '—'

  const profitData = useMemo(() => {
    const formatter = new Intl.DateTimeFormat('ru-RU', {
      hour: '2-digit',
      minute: '2-digit',
    })
    const buckets = new Map<string, { time: string, surebets: number, avgProfitTotal: number }>()

    surebets.slice(0, 120).forEach((surebet) => {
      const detectedAt = new Date(surebet.detected_at)
      if (Number.isNaN(detectedAt.getTime())) return

      detectedAt.setMinutes(0, 0, 0)
      const key = detectedAt.toISOString()
      const bucket = buckets.get(key) ?? { time: formatter.format(detectedAt), surebets: 0, avgProfitTotal: 0 }
      bucket.surebets += 1
      bucket.avgProfitTotal += surebet.profit_percent
      buckets.set(key, bucket)
    })

    return [...buckets.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .slice(-12)
      .map(([, bucket]) => ({
        time: bucket.time,
        surebets: bucket.surebets,
        avgProfit: bucket.surebets > 0 ? bucket.avgProfitTotal / bucket.surebets : 0,
      }))
  }, [surebets])

  const marketData = useMemo(() => {
    const counts = new Map<string, number>()

    surebets.forEach((surebet) => {
      const market = surebet.legs[0]?.market?.trim() || 'Прочее'
      counts.set(market, (counts.get(market) ?? 0) + 1)
    })

    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, 6)
      .map(([market, count], index) => ({
        market,
        count,
        color: chartPalette[index % chartPalette.length],
      }))
  }, [surebets])

  const topValueBets = useMemo(() => {
    return [...valueBets]
      .sort((a, b) => b.edge_percent - a.edge_percent)
      .slice(0, 5)
  }, [valueBets])

  const topGenerosity = useMemo(() => {
    return [...generosityIndices]
      .sort((a, b) => b.score - a.score)
      .slice(0, 5)
  }, [generosityIndices])

  const bestValueEdge = topValueBets[0]?.edge_percent ?? 0
  const bestGenerosityScore = topGenerosity[0]?.score ?? 0
  const executionStatus = executionOverview?.autobet_status ?? null
  const executionAccounts = executionOverview?.accounts ?? null
  const executionPlacements = executionOverview?.recent_placements ?? null
  const executionLedger = executionOverview?.ledger_placements ?? null
  const executionReadyRate = executionAccounts && executionAccounts.total_bookmakers > 0
    ? (executionAccounts.ready_for_execution / executionAccounts.total_bookmakers) * 100
    : 0
  const placementErrorRate = executionPlacements && executionPlacements.total > 0
    ? (executionPlacements.errors / executionPlacements.total) * 100
    : 0
  const pendingPlacements = executionPlacements?.pending ?? 0

  const executionTone = executionStatus?.emergency_stopped
    ? { label: 'Execution остановлен', badge: 'badge-danger', icon: ShieldX, accent: 'var(--accent-red)' }
    : executionStatus?.running
      ? { label: 'AutoBet активен', badge: 'badge-success', icon: PlayCircle, accent: 'var(--accent-green)' }
      : executionStatus?.enabled
        ? { label: 'Execution в standby', badge: 'badge-warning', icon: PauseCircle, accent: 'var(--accent-yellow)' }
        : { label: 'Execution выключен', badge: 'badge-info', icon: PauseCircle, accent: 'var(--accent-blue)' }
  const ExecutionToneIcon = executionTone.icon

  const executionAlerts = useMemo(() => {
    if (!executionOverview) return [] as string[]

    const alerts: string[] = []
    if (executionOverview.autobet_status.emergency_stopped) alerts.push('Снят флаг emergency stop перед любым armed режимом.')
    if ((executionOverview.accounts.accounts_with_control_issues ?? 0) > 0) alerts.push(`Control issues: ${executionOverview.accounts.accounts_with_control_issues} аккаунтов требуют внимания.`)
    if ((executionOverview.accounts.ready_for_execution ?? 0) === 0) alerts.push('Нет аккаунтов, готовых к execution path.')
    if ((executionOverview.recent_placements.pending ?? 0) > 0) alerts.push(`Есть pending placements: ${executionOverview.recent_placements.pending}.`)
    if ((executionOverview.recent_placements.errors ?? 0) > 0) alerts.push(`Execution errors в последних размещениях: ${executionOverview.recent_placements.errors}.`)
    if (alerts.length === 0) alerts.push('Execution surface выглядит чисто: можно держать dry-run и ledger под мониторингом.')

    return alerts.slice(0, 3)
  }, [executionOverview])

  const formatDateTime = (value: string | null) => {
    if (!value) return '—'

    const date = new Date(value)
    return Number.isNaN(date.getTime())
      ? '—'
      : date.toLocaleString('ru-RU', {
        day: '2-digit',
        month: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
      })
  }

  const formatCurrency = (value: number) => `${value >= 0 ? '+' : ''}${value.toLocaleString('ru-RU', { maximumFractionDigits: 2 })} RUB`

  const statCards = [
    {
      label: 'Вилок найдено',
      value: realSurebetsFound.toLocaleString(),
      change: realSurebetsFound > 0 ? 'в кэше' : 'поиск...',
      icon: Zap,
      gradient: 'linear-gradient(135deg, #3fb950 0%, #39d2c0 100%)',
      suffix: ''
    },
    {
      label: 'Событий обработано',
      value: realEventsParsed.toLocaleString(),
      change: 'за цикл',
      icon: Target,
      gradient: 'linear-gradient(135deg, #58a6ff 0%, #bc8cff 100%)',
      suffix: ''
    },
    {
      label: 'Букмекеров',
      value: activeBKs.toString(),
      change: 'подключено',
      icon: Users,
      gradient: 'linear-gradient(135deg, #bc8cff 0%, #f778ba 100%)',
      suffix: ''
    },
    {
      label: 'Время цикла',
      value: cycleSeconds,
      change: metrics ? 'секунд' : '',
      icon: Clock,
      gradient: 'linear-gradient(135deg, #d29922 0%, #f0883e 100%)',
      suffix: 's'
    },
  ]

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-2xl font-bold">Обзор системы</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Мониторинг арбитражных возможностей в реальном времени
          </p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            <div className="w-2 h-2 rounded-full glow-live" style={{ background: 'var(--accent-green)' }} />
            <span className="text-xs font-medium">Сканер активен</span>
          </div>
        </div>
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {statCards.map((stat, i) => {
          const Icon = stat.icon
          const isPositive = stat.change.startsWith('+')
          
          return (
            <motion.div key={i} variants={item} className="stat-card">
              <div className="flex items-start justify-between mb-4">
                <div>
                  <p className="text-xs font-medium uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>
                    {stat.label}
                  </p>
                  <p className="text-3xl font-bold mt-1">
                    {stat.value}<span className="text-lg ml-1" style={{ color: 'var(--text-secondary)' }}>{stat.suffix}</span>
                  </p>
                </div>
                <div className="w-12 h-12 rounded-xl flex items-center justify-center" style={{ background: stat.gradient }}>
                  <Icon size={24} color="#fff" />
                </div>
              </div>
              
              <div className="flex items-center gap-1.5">
                {isPositive ? (
                  <ArrowUpRight size={14} color="var(--accent-green)" />
                ) : (
                  <ArrowDownRight size={14} color="var(--accent-red)" />
                )}
                <span className={`text-xs font-medium ${isPositive ? 'profit-positive' : 'profit-negative'}`}>
                  {stat.change}
                </span>
              </div>
            </motion.div>
          )
        })}
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">
        <motion.div variants={item} className="glass-card p-5 xl:col-span-2">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Execution overview</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Операционный срез по `/api/v1/execution/overview`</p>
            </div>
            <span className={`badge ${executionTone.badge}`}>{executionTone.label}</span>
          </div>

          {executionOverview ? (
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex items-center justify-between mb-3">
                  <div>
                    <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Autobet posture</p>
                    <p className="text-lg font-semibold mt-1">{executionStatus?.running ? 'Running' : executionStatus?.enabled ? 'Standby' : 'Disabled'}</p>
                  </div>
                  <ExecutionToneIcon size={20} style={{ color: executionTone.accent }} />
                </div>
                <div className="space-y-2 text-sm">
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Сегодня</span><span>{executionStatus?.bets_placed_today ?? 0} bets</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Ошибки</span><span className={executionStatus && executionStatus.errors_today > 0 ? 'profit-negative' : ''}>{executionStatus?.errors_today ?? 0}</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>PnL day</span><span className={executionStatus && executionStatus.profit_today >= 0 ? 'profit-positive' : 'profit-negative'}>{formatCurrency(executionStatus?.profit_today ?? 0)}</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Last bet</span><span>{formatDateTime(executionStatus?.last_bet ?? null)}</span></div>
                </div>
              </div>

              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex items-center justify-between mb-3">
                  <div>
                    <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Accounts readiness</p>
                    <p className="text-lg font-semibold mt-1">{executionAccounts?.ready_for_execution ?? 0} / {executionAccounts?.total_bookmakers ?? 0}</p>
                  </div>
                  <Wallet size={20} style={{ color: 'var(--accent-blue)' }} />
                </div>
                <div className="space-y-2 text-sm">
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Ready for execution</span><span>{executionReadyRate.toFixed(0)}%</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Sessions auth</span><span>{executionAccounts?.sessions_authenticated ?? 0} / {executionAccounts?.sessions_configured ?? 0}</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Balances cached</span><span>{executionAccounts?.balances_cached ?? 0}</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Control issues</span><span className={executionAccounts && executionAccounts.accounts_with_control_issues > 0 ? 'profit-negative' : 'profit-positive'}>{executionAccounts?.accounts_with_control_issues ?? 0}</span></div>
                </div>
              </div>

              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex items-center justify-between mb-3">
                  <div>
                    <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Placement flow</p>
                    <p className="text-lg font-semibold mt-1">{executionPlacements?.placed ?? 0} placed</p>
                  </div>
                  <Activity size={20} style={{ color: 'var(--accent-cyan)' }} />
                </div>
                <div className="space-y-2 text-sm">
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Recent total</span><span>{executionPlacements?.total ?? 0}</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Pending / settled</span><span>{executionPlacements?.pending ?? 0} / {executionPlacements?.settled ?? 0}</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Error rate</span><span className={placementErrorRate > 0 ? 'profit-negative' : 'profit-positive'}>{placementErrorRate.toFixed(1)}%</span></div>
                  <div className="flex items-center justify-between"><span style={{ color: 'var(--text-secondary)' }}>Ledger total</span><span>{executionLedger?.total ?? 0}</span></div>
                </div>
              </div>
            </div>
          ) : (
            <div className="rounded-xl border border-dashed p-6 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
              <PauseCircle size={28} className="mx-auto mb-3 opacity-30" />
              <p className="text-sm">Execution overview ещё не доступен от backend, но surface уже готов к контракту.</p>
            </div>
          )}
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">Operator notes</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Что проверить перед включением исполнения</p>
            </div>
            <AlertTriangle size={16} style={{ color: 'var(--accent-yellow)' }} />
          </div>

          <div className="space-y-3">
            {executionAlerts.map((alert) => (
              <div key={alert} className="rounded-lg p-3" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <p className="text-sm">{alert}</p>
              </div>
            ))}

            <div className="rounded-lg p-3" style={{ background: 'rgba(88, 166, 255, 0.08)', border: '1px solid rgba(88, 166, 255, 0.2)' }}>
              <p className="text-xs uppercase tracking-wider mb-1" style={{ color: 'var(--text-muted)' }}>Snapshot</p>
              <p className="text-sm">Generated {formatDateTime(executionOverview?.generated_at ?? null)}</p>
              <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                {pendingPlacements > 0 ? `Очередь требует внимания: ${pendingPlacements} pending.` : 'Pending queue чистая или ещё не пришла.'}
              </p>
            </div>
          </div>
        </motion.div>
      </div>

      <motion.div variants={item}>
        <ParserDeepDive parserCoverage={parserCoverage} parserHealth={parserHealth} />
      </motion.div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">Value bets</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Топ по `/api/v1/value-bets`</p>
            </div>
            <div className="text-right">
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Лучший edge</p>
              <p className="text-lg font-semibold" style={{ color: 'var(--accent-green)' }}>+{bestValueEdge.toFixed(2)}%</p>
            </div>
          </div>

          <div className="space-y-2">
            {topValueBets.map((bet) => (
              <div
                key={bet.id}
                className="flex items-start justify-between gap-3 p-3 rounded-lg"
                style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}
              >
                <div>
                  <p className="text-sm font-medium">{bet.event.home_team} — {bet.event.away_team}</p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                    {bet.bookmaker} • {bet.market} / {bet.selection}
                  </p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>
                    fair {bet.fair_odds.toFixed(2)} • detected {formatDateTime(bet.detected_at)}
                  </p>
                </div>

                <div className="text-right shrink-0">
                  <p className="profit profit-positive text-base">+{bet.edge_percent.toFixed(2)}%</p>
                  <p className="text-xs font-mono mt-1">@ {bet.odds.toFixed(2)}</p>
                </div>
              </div>
            ))}

            {topValueBets.length === 0 && (
              <div className="text-center py-10" style={{ color: 'var(--text-muted)' }}>
                <BadgePercent size={40} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Value bets пока не пришли</p>
              </div>
            )}
          </div>
        </motion.div>

        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">Generosity</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Срез по `/api/v1/analytics/generosity`</p>
            </div>
            <div className="text-right">
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Лучший score</p>
              <p className="text-lg font-semibold" style={{ color: 'var(--accent-blue)' }}>{bestGenerosityScore.toFixed(2)}</p>
            </div>
          </div>

          <div className="space-y-2">
            {topGenerosity.map((entry) => (
              <div
                key={`${entry.bookmaker}-${entry.sport}`}
                className="flex items-start justify-between gap-3 p-3 rounded-lg"
                style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}
              >
                <div>
                  <p className="text-sm font-medium capitalize">{entry.bookmaker}</p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                    {entry.sport} • events {entry.total_events} • best odds {entry.best_odds_count}
                  </p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>
                    margin {entry.avg_margin.toFixed(2)}% • avg odds {entry.avg_odds.toFixed(2)}
                  </p>
                </div>

                <div className="text-right shrink-0">
                  <p className="text-base font-semibold">{entry.score.toFixed(2)}</p>
                  <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>{formatDateTime(entry.updated_at)}</p>
                </div>
              </div>
            ))}

            {topGenerosity.length === 0 && (
              <div className="text-center py-10" style={{ color: 'var(--text-muted)' }}>
                <Gem size={40} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Generosity snapshot пока пуст</p>
              </div>
            )}
          </div>
        </motion.div>
      </div>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Activity Chart */}
        <motion.div variants={item} className="lg:col-span-2 chart-container p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">Активность вилок</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Последние часы по реальным находкам</p>
            </div>
            <div className="flex items-center gap-2">
              <BarChart3 size={16} style={{ color: 'var(--text-muted)' }} />
            </div>
          </div>
          
          {profitData.length > 0 ? (
            <ResponsiveContainer width="100%" height={280}>
              <AreaChart data={profitData}>
                <defs>
                  <linearGradient id="surebetGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#58a6ff" stopOpacity={0.3} />
                    <stop offset="100%" stopColor="#58a6ff" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <XAxis 
                  dataKey="time" 
                  tick={{ fill: '#484f58', fontSize: 11 }} 
                  axisLine={false} 
                  tickLine={false}
                />
                <YAxis 
                  tick={{ fill: '#484f58', fontSize: 11 }} 
                  axisLine={false} 
                  tickLine={false}
                  allowDecimals={false}
                />
                <Tooltip
                  contentStyle={{ 
                    background: '#161b22', 
                    border: '1px solid rgba(255,255,255,0.06)',
                    borderRadius: '10px',
                    boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
                  }}
                  labelStyle={{ color: '#e6edf3', marginBottom: '4px' }}
                />
                <Area 
                  type="monotone" 
                  dataKey="surebets" 
                  stroke="#58a6ff" 
                  strokeWidth={2}
                  fill="url(#surebetGradient)"
                />
              </AreaChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[280px] flex items-center justify-center text-center" style={{ color: 'var(--text-muted)' }}>
              <div>
                <BarChart3 size={40} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">График появится после первых вилок</p>
              </div>
            </div>
          )}
        </motion.div>

        {/* Market Distribution */}
        <motion.div variants={item} className="chart-container p-5">
          <h3 className="text-base font-semibold mb-1">По рынкам</h3>
          <p className="text-xs mb-4" style={{ color: 'var(--text-muted)' }}>Распределение вилок</p>
          
          {marketData.length > 0 ? (
            <ResponsiveContainer width="100%" height={280}>
              <BarChart data={marketData} layout="vertical">
                <XAxis type="number" hide />
                <YAxis 
                  dataKey="market" 
                  type="category" 
                  tick={{ fill: '#8b949e', fontSize: 12 }} 
                  axisLine={false}
                  tickLine={false}
                  width={70}
                />
                <Tooltip
                  contentStyle={{ 
                    background: '#161b22', 
                    border: '1px solid rgba(255,255,255,0.06)',
                    borderRadius: '10px',
                  }}
                />
                <Bar dataKey="count" radius={[0, 6, 6, 0]} barSize={24}>
                  {marketData.map((entry, i) => (
                    <Cell key={i} fill={entry.color} />
                  ))}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-[280px] flex items-center justify-center text-center" style={{ color: 'var(--text-muted)' }}>
              <div>
                <TrendingUp size={40} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Распределение рынков появится после загрузки данных</p>
              </div>
            </div>
          )}
        </motion.div>
      </div>

      {/* Recent Surebets & Bookmakers */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Top Surebets */}
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">Топ вилки</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Самые прибыльные за сегодня</p>
            </div>
            <Target size={16} style={{ color: 'var(--text-muted)' }} />
          </div>
          
          <div className="space-y-2">
            {surebets.slice(0, 5).map((sb, i) => (
              <motion.div 
                key={sb.id} 
                className="flex items-center justify-between p-3 rounded-lg transition-all duration-200 cursor-pointer"
                style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}
                whileHover={{ background: 'var(--bg-hover)', scale: 1.01 }}
              >
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-lg flex items-center justify-center text-xs font-bold"
                       style={{ background: 'var(--gradient-primary)', color: '#fff' }}>
                    {i + 1}
                  </div>
                  <div>
                    <p className="text-sm font-medium">{sb.home_team} — {sb.away_team}</p>
                    <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{sb.league}</p>
                  </div>
                </div>
                <div className="text-right">
                  <p className="profit profit-positive text-base">+{sb.profit_percent.toFixed(2)}%</p>
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                    {sb.legs[0]?.bookmaker} / {sb.legs[1]?.bookmaker}
                  </p>
                </div>
              </motion.div>
            ))}
            
            {surebets.length === 0 && (
              <div className="text-center py-12" style={{ color: 'var(--text-muted)' }}>
                <Zap size={48} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Вилок пока нет</p>
                <p className="text-xs mt-1">Сканер продолжает поиск...</p>
              </div>
            )}
          </div>
        </motion.div>

        {/* Bookmakers Status */}
        <motion.div variants={item} className="glass-card p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">Букмекеры</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Статус подключений</p>
            </div>
            <Users size={16} style={{ color: 'var(--text-muted)' }} />
          </div>
          
          <div className="space-y-2">
            {bookmakers.map((bk, i) => (
              <motion.div 
                key={bk.name} 
                className="flex items-center justify-between p-3 rounded-lg transition-all duration-200"
                style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}
                whileHover={{ background: 'var(--bg-hover)' }}
              >
                <div className="flex items-center gap-3">
                  <div
                    className={`w-2 h-2 rounded-full ${bk.status === 'active' ? 'glow-live' : ''}`}
                    style={{
                      background:
                        bk.status === 'active'
                          ? 'var(--accent-green)'
                          : bk.status === 'inactive'
                            ? 'var(--accent-yellow)'
                            : 'var(--accent-red)'
                    }}
                  />
                  <div>
                    <p className="text-sm font-medium capitalize">{bk.name}</p>
                    <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                      {bk.backend_status === 'execution_ready'
                        ? 'готов к исполнению'
                        : bk.backend_status === 'scan_only'
                          ? 'только сканирование'
                          : bk.notes || 'отключен'}
                    </p>
                  </div>
                </div>
                <span className={`badge ${bk.status === 'active' ? 'badge-success' : bk.status === 'inactive' ? 'badge-warning' : 'badge-danger'}`}>
                  {bk.status === 'active' ? 'Active' : bk.status === 'inactive' ? 'Inactive' : 'Error'}
                </span>
              </motion.div>
            ))}

            {bookmakers.length === 0 && (
              <div className="text-center py-12" style={{ color: 'var(--text-muted)' }}>
                <Users size={48} className="mx-auto mb-3 opacity-20" />
                <p className="text-sm">Список букмекеров загрузится из API</p>
              </div>
            )}
          </div>
        </motion.div>
      </div>
    </motion.div>
  )
}
