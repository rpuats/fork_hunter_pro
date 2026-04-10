import { motion } from 'framer-motion'
import { Zap, TrendingUp, Users, Clock, Target, BarChart3, ArrowUpRight, ArrowDownRight } from 'lucide-react'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, BarChart, Bar, Cell } from 'recharts'
import type { ScannerMetrics, Surebet, Bookmaker } from '../types'

interface DashboardProps {
  metrics: ScannerMetrics | null
  surebets: Surebet[]
  bookmakers: Bookmaker[]
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

// Mock data for charts
const profitData = [
  { time: '00:00', surebets: 12, avgProfit: 1.2 },
  { time: '02:00', surebets: 8, avgProfit: 0.9 },
  { time: '04:00', surebets: 5, avgProfit: 0.7 },
  { time: '06:00', surebets: 15, avgProfit: 1.5 },
  { time: '08:00', surebets: 28, avgProfit: 2.1 },
  { time: '10:00', surebets: 42, avgProfit: 2.8 },
  { time: '12:00', surebets: 56, avgProfit: 3.2 },
  { time: '14:00', surebets: 48, avgProfit: 2.9 },
  { time: '16:00', surebets: 35, avgProfit: 2.3 },
  { time: '18:00', surebets: 45, avgProfit: 2.7 },
  { time: '20:00', surebets: 38, avgProfit: 2.4 },
  { time: '22:00', surebets: 22, avgProfit: 1.8 },
]

const marketData = [
  { market: '1X2', count: 45, color: '#58a6ff' },
  { market: 'Тотал', count: 28, color: '#bc8cff' },
  { market: 'Фора', count: 18, color: '#3fb950' },
  { market: 'ОЗ', count: 12, color: '#39d2c0' },
  { market: 'Чёт/Нечет', count: 8, color: '#d29922' },
  { market: 'Двойной', count: 6, color: '#f85149' },
]

export function Dashboard({ metrics, surebets, bookmakers }: DashboardProps) {
  // Используем реальные метрики сканнера, если они есть
  const realSurebetsFound = metrics?.surebets_found ?? 0
  const realEventsParsed = metrics?.events_parsed ?? 0
  const activeBKs = bookmakers.filter(b => b.status === 'active').length

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
      value: metrics ? `${(metrics.cycle_time_ms / 1000).toFixed(1)}` : '—',
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

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Activity Chart */}
        <motion.div variants={item} className="lg:col-span-2 chart-container p-5">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="text-base font-semibold">Активность вилок</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>За последние 24 часа</p>
            </div>
            <div className="flex items-center gap-2">
              <BarChart3 size={16} style={{ color: 'var(--text-muted)' }} />
            </div>
          </div>
          
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
        </motion.div>

        {/* Market Distribution */}
        <motion.div variants={item} className="chart-container p-5">
          <h3 className="text-base font-semibold mb-1">По рынкам</h3>
          <p className="text-xs mb-4" style={{ color: 'var(--text-muted)' }}>Распределение вилок</p>
          
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
            {(bookmakers.length > 0 ? bookmakers : [
              { name: 'Pari', events: 6608, status: 'active' as const },
              { name: 'Fonbet', events: 6826, status: 'active' as const },
              { name: 'Bettery', events: 6843, status: 'active' as const },
              { name: 'Marathon', events: 6566, status: 'active' as const },
              { name: '24bet', events: 6557, status: 'active' as const },
              { name: 'Leon', events: 3676, status: 'active' as const },
              { name: 'Sportbet', events: 258, status: 'active' as const },
            ]).map((bk: any, i) => (
              <motion.div 
                key={bk.name} 
                className="flex items-center justify-between p-3 rounded-lg transition-all duration-200"
                style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}
                whileHover={{ background: 'var(--bg-hover)' }}
              >
                <div className="flex items-center gap-3">
                  <div className="w-2 h-2 rounded-full glow-live" style={{ background: 'var(--accent-green)' }} />
                  <div>
                    <p className="text-sm font-medium capitalize">{bk.name}</p>
                    <p className="text-xs" style={{ color: 'var(--text-muted)' }}>
                      {bk.events?.toLocaleString()} событий
                    </p>
                  </div>
                </div>
                <span className="badge badge-success">
                  Active
                </span>
              </motion.div>
            ))}
          </div>
        </motion.div>
      </div>
    </motion.div>
  )
}
