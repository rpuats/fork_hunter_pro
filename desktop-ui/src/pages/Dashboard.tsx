import { motion } from 'framer-motion'
import { 
  Zap, TrendingUp, BarChart3, Flame, DollarSign, Percent, Timer, RefreshCw,
  FileText, Settings, Bell, CheckCircle2, XCircle, ChevronRight, Activity,
  AlertTriangle, Wallet, Target
} from 'lucide-react'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'
import { StatCard } from '../components/StatCard'
import { demoForks, demoNotifications, demoProfitChart, demoAccounts } from '../lib/demoData'

// Format money
const formatMoney = (amount: number) => new Intl.NumberFormat('ru-RU').format(amount) + ' ₽'

// Format relative time
const formatRelativeTime = (dateStr: string | null) => {
  if (!dateStr) return '—'
  const diff = Math.floor((Date.now() - new Date(dateStr).getTime()) / 60000)
  if (diff < 1) return 'только что'
  if (diff < 60) return `${diff} мин назад`
  const hours = Math.floor(diff / 60)
  if (hours < 24) return `${hours} ч назад`
  return `${Math.floor(hours / 24)} дн назад`
}

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

const profitData = demoProfitChart.labels.map((label, i) => ({
  time: label,
  profit: demoProfitChart.data[i]
}))

export function Dashboard() {
  const activeForks = demoForks.filter(f => f.isHot)
  const connectedAccounts = demoAccounts.filter(a => a.connectionStatus === 'connected')
  const totalBankroll = demoAccounts.reduce((sum, a) => sum + a.balance, 0)
  const totalProfit = demoAccounts.reduce((sum, a) => sum + a.totalProfit, 0)

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 space-y-6">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">Обзор</h1>
          <p className="text-sm text-text-secondary mt-1">Статистика и активность сканера</p>
        </div>
        <div className="flex items-center gap-2 text-xs text-text-muted">
          <RefreshCw size={14} />
          Обновлено {formatRelativeTime(new Date().toISOString())}
        </div>
      </motion.div>

      {/* Stats Row */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div variants={item}>
          <StatCard
            icon={DollarSign}
            label="Профит сегодня"
            value={formatMoney(2450)}
            trend="12%"
            trendUp={true}
            color="green"
          />
        </motion.div>
        <motion.div variants={item}>
          <StatCard
            icon={Target}
            label="Вилки найдено"
            value={demoForks.length}
            trend={`${activeForks.length} активные`}
            color="blue"
          />
        </motion.div>
        <motion.div variants={item}>
          <StatCard
            icon={Percent}
            label="ROI"
            value="4.2%"
            trend="За 7 дней"
            color="purple"
          />
        </motion.div>
        <motion.div variants={item}>
          <StatCard
            icon={Timer}
            label="Скорость"
            value="0.3с"
            trend="-0.1с"
            trendUp={true}
            color="orange"
          />
        </motion.div>
      </div>

      {/* Main Content - 2 columns */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left column - 2/3 */}
        <div className="lg:col-span-2 space-y-6">
          {/* Profit Chart */}
          <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
            <div className="flex items-center justify-between mb-4">
              <div>
                <h3 className="text-base font-semibold text-text-primary">Динамика профита</h3>
                <p className="text-xs text-text-muted mt-0.5">Последние 7 дней</p>
              </div>
              <BarChart3 size={16} className="text-text-muted" />
            </div>
            <ResponsiveContainer width="100%" height={260}>
              <AreaChart data={profitData}>
                <defs>
                  <linearGradient id="profitGradient" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#10B981" stopOpacity={0.3} />
                    <stop offset="100%" stopColor="#10B981" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <XAxis dataKey="time" tick={{ fill: '#64748B', fontSize: 11 }} axisLine={false} tickLine={false} />
                <YAxis tick={{ fill: '#64748B', fontSize: 11 }} axisLine={false} tickLine={false} />
                <Tooltip
                  contentStyle={{ background: '#151A25', border: '1px solid #2A3142', borderRadius: '12px' }}
                  labelStyle={{ color: '#F1F5F9', marginBottom: '4px' }}
                />
                <Area type="monotone" dataKey="profit" stroke="#10B981" strokeWidth={2} fill="url(#profitGradient)" />
              </AreaChart>
            </ResponsiveContainer>
          </motion.div>

          {/* Hot Forks */}
          <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-2">
                <Flame size={18} className="text-orange-400" />
                <h3 className="text-base font-semibold text-text-primary">Горячие вилки</h3>
              </div>
              <button className="text-xs text-accent hover:text-accent-hover flex items-center gap-1">
                Все вилки <ChevronRight size={14} />
              </button>
            </div>
            <div className="space-y-3">
              {activeForks.map((fork) => (
                <div key={fork.id} className="flex items-center gap-4 p-3 rounded-lg bg-background hover:bg-elevated/50 transition-colors cursor-pointer group">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-semibold text-text-primary truncate">{fork.match}</span>
                      {fork.isHot && <Flame size={14} className="text-orange-400 shrink-0" />}
                    </div>
                    <div className="text-xs text-text-secondary mt-0.5">{fork.league} • {fork.market}</div>
                  </div>
                  <div className="text-right shrink-0">
                    <div className="text-sm font-bold text-emerald-400">+{fork.profit}%</div>
                    <div className="text-xs text-text-muted">{formatMoney(fork.totalStake)}</div>
                  </div>
                  <div className="flex gap-1 shrink-0">
                    {fork.bookmakers.map((bk, i) => (
                      <span key={i} className="text-xs px-2 py-1 rounded bg-surface border border-border text-text-secondary">
                        {bk.name}
                      </span>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </motion.div>
        </div>

        {/* Right column - 1/3 */}
        <div className="space-y-6">
          {/* Bookmaker Status */}
          <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
            <h3 className="text-base font-semibold text-text-primary mb-4">Букмекеры</h3>
            <div className="space-y-3">
              {demoAccounts.map((account) => (
                <div key={account.id} className="flex items-center gap-3">
                  <div className={`w-2 h-2 rounded-full ${account.connectionStatus === 'connected' ? 'bg-emerald-400' : account.connectionStatus === 'error' ? 'bg-red-400' : 'bg-amber-400'}`} />
                  <div className="flex-1">
                    <div className="text-sm font-medium text-text-primary">{account.name}</div>
                    <div className="text-xs text-text-secondary">{formatMoney(account.balance)}</div>
                  </div>
                  {account.riskFactors.length > 0 && <AlertTriangle size={14} className="text-amber-400" />}
                  {account.bonuses.length > 0 && <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-purple-500/20 text-purple-400">{account.bonuses.length}</span>}
                </div>
              ))}
            </div>
          </motion.div>

          {/* Quick Actions */}
          <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
            <h3 className="text-base font-semibold text-text-primary mb-4">Быстрые действия</h3>
            <div className="space-y-2">
              <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg bg-background hover:bg-elevated/50 transition-colors text-left">
                <RefreshCw size={16} className="text-accent" />
                <span className="text-sm text-text-primary">Обновить балансы</span>
              </button>
              <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg bg-background hover:bg-elevated/50 transition-colors text-left">
                <FileText size={16} className="text-accent" />
                <span className="text-sm text-text-primary">Отчёт за неделю</span>
              </button>
              <button className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg bg-background hover:bg-elevated/50 transition-colors text-left">
                <Settings size={16} className="text-accent" />
                <span className="text-sm text-text-primary">Настройки</span>
              </button>
            </div>
          </motion.div>

          {/* Notifications */}
          <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-base font-semibold text-text-primary">Уведомления</h3>
              <Bell size={16} className="text-text-muted" />
            </div>
            <div className="space-y-3">
              {demoNotifications.slice(0, 3).map((note) => (
                <div key={note.id} className="flex items-start gap-2">
                  {note.type === 'success' && <CheckCircle2 size={14} className="text-emerald-400 shrink-0 mt-0.5" />}
                  {note.type === 'warning' && <AlertTriangle size={14} className="text-amber-400 shrink-0 mt-0.5" />}
                  {note.type === 'error' && <XCircle size={14} className="text-red-400 shrink-0 mt-0.5" />}
                  {note.type === 'info' && <Activity size={14} className="text-blue-400 shrink-0 mt-0.5" />}
                  <div className="flex-1 min-w-0">
                    <div className="text-xs text-text-primary">{note.message}</div>
                    <div className="text-[10px] text-text-muted mt-0.5">{formatRelativeTime(note.time)}</div>
                  </div>
                </div>
              ))}
            </div>
          </motion.div>

          {/* Total Bankroll */}
          <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
            <div className="flex items-center gap-2 mb-2">
              <Wallet size={16} className="text-accent" />
              <span className="text-sm text-text-secondary">Общий банкролл</span>
            </div>
            <div className="text-2xl font-bold text-text-primary">{formatMoney(totalBankroll)}</div>
            <div className="text-xs text-emerald-400 mt-1">+{formatMoney(totalProfit)} за всё время</div>
          </motion.div>
        </div>
      </div>
    </motion.div>
  )
}
