import { useState } from 'react'
import { motion } from 'framer-motion'
import { 
  History, Filter, Download, Search, Calendar, TrendingUp,
  TrendingDown, CheckCircle2, XCircle, Clock, Zap, Target,
  FileText, ChevronDown, BarChart3
} from 'lucide-react'
import { StatCard } from '../components/StatCard'

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

const demoHistory = [
  { id: 1, type: 'surebet', match: 'ЦСКА — Спартак', bookmakers: ['Pari', 'Fonbet'], profit: 520, amount: 10000, date: '2026-01-15 14:30', status: 'win' },
  { id: 2, type: 'surebet', match: 'Реал Мадрид — Барселона', bookmakers: ['Fonbet', 'Leon'], profit: 1050, amount: 15000, date: '2026-01-15 12:15', status: 'win' },
  { id: 3, type: 'corridor', match: 'Зенит — Локомотив', bookmakers: ['Pari', 'Winline'], profit: 0, amount: 8000, date: '2026-01-14 18:45', status: 'push' },
  { id: 4, type: 'express', match: 'Экспресс x3', bookmakers: ['Pari'], profit: -5000, amount: 5000, date: '2026-01-14 16:20', status: 'loss' },
  { id: 5, type: 'surebet', match: 'Ман Сити — Ливерпуль', bookmakers: ['Leon', 'Fonbet'], profit: 340, amount: 5000, date: '2026-01-14 10:00', status: 'win' },
  { id: 6, type: 'surebet', match: 'Динамо — Краснодар', bookmakers: ['Pari', 'Winline'], profit: 280, amount: 4000, date: '2026-01-13 20:30', status: 'win' },
  { id: 7, type: 'corridor', match: 'ЦСКА — Зенит', bookmakers: ['Fonbet', 'Leon'], profit: 640, amount: 5000, date: '2026-01-13 15:00', status: 'win' },
]

const formatMoney = (amount: number) => new Intl.NumberFormat('ru-RU').format(amount) + ' ₽'

const typeLabels: Record<string, string> = {
  surebet: 'Вилка',
  corridor: 'Коридор',
  express: 'Экспресс'
}

const typeIcons: Record<string, any> = {
  surebet: Zap,
  corridor: Target,
  express: FileText
}

export function HistoryPage() {
  const [filter, setFilter] = useState<'all' | 'win' | 'loss' | 'push'>('all')
  const [typeFilter, setTypeFilter] = useState<'all' | 'surebet' | 'corridor' | 'express'>('all')
  const [search, setSearch] = useState('')

  const filtered = demoHistory.filter(item => {
    if (filter !== 'all' && item.status !== filter) return false
    if (typeFilter !== 'all' && item.type !== typeFilter) return false
    if (search && !item.match.toLowerCase().includes(search.toLowerCase())) return false
    return true
  })

  const stats = {
    totalBets: demoHistory.length,
    wins: demoHistory.filter(i => i.status === 'win').length,
    losses: demoHistory.filter(i => i.status === 'loss').length,
    pushes: demoHistory.filter(i => i.status === 'push').length,
    totalProfit: demoHistory.reduce((sum, i) => sum + i.profit, 0),
    winRate: Math.round((demoHistory.filter(i => i.status === 'win').length / demoHistory.length) * 100)
  }

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 space-y-6">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">История ставок</h1>
          <p className="text-sm text-text-secondary mt-1">{filtered.length} записей • Период: 7 дней</p>
        </div>
        <button className="btn btn-secondary text-sm flex items-center gap-2">
          <Download size={16} /> Экспорт CSV
        </button>
      </motion.div>

      {/* Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div variants={item}>
          <StatCard icon={History} label="Всего ставок" value={stats.totalBets} color="blue" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={TrendingUp} label="Выигрышей" value={stats.wins} color="green" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={TrendingDown} label="Проигрышей" value={stats.losses} color="red" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard 
            icon={BarChart3} 
            label="Общий профит" 
            value={formatMoney(stats.totalProfit)} 
            color={stats.totalProfit >= 0 ? 'green' : 'red'} 
          />
        </motion.div>
      </div>

      {/* Filters */}
      <motion.div variants={item} className="flex flex-wrap items-center gap-3">
        <div className="relative">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted" />
          <input 
            type="text" 
            placeholder="Поиск..."
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="input pl-9 w-64"
          />
        </div>

        <div className="flex items-center gap-2 p-1 rounded-lg bg-surface border border-border">
          {(['all', 'win', 'loss', 'push'] as const).map(f => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1.5 rounded-md text-sm transition-colors ${
                filter === f 
                  ? 'bg-accent text-white' 
                  : 'text-text-secondary hover:text-text-primary'
              }`}
            >
              {f === 'all' ? 'Все' : f === 'win' ? 'Выигрыши' : f === 'loss' ? 'Проигрыши' : 'Возвраты'}
            </button>
          ))}
        </div>

        <select 
          value={typeFilter} 
          onChange={e => setTypeFilter(e.target.value as any)}
          className="input"
        >
          <option value="all">Все типы</option>
          <option value="surebet">Вилки</option>
          <option value="corridor">Коридоры</option>
          <option value="express">Экспрессы</option>
        </select>
      </motion.div>

      {/* History Table */}
      <motion.div variants={item} className="rounded-card border border-border bg-surface overflow-hidden">
        {/* Header */}
        <div className="grid grid-cols-12 gap-4 px-5 py-3 bg-background border-b border-border text-xs text-text-secondary uppercase tracking-wider">
          <div className="col-span-2">Тип</div>
          <div className="col-span-3">Событие</div>
          <div className="col-span-2">БК</div>
          <div className="col-span-2 text-right">Сумма</div>
          <div className="col-span-2 text-right">Результат</div>
          <div className="col-span-1 text-right">Дата</div>
        </div>

        {/* Body */}
        <div className="divide-y divide-border/50">
          {filtered.map((item) => {
            const TypeIcon = typeIcons[item.type]
            return (
              <motion.div 
                key={item.id}
                className="grid grid-cols-12 gap-4 px-5 py-4 items-center hover:bg-elevated/30 transition-colors"
                whileHover={{ x: 2 }}
              >
                {/* Type */}
                <div className="col-span-2">
                  <div className="flex items-center gap-2">
                    <div className={`p-1.5 rounded-lg ${
                      item.type === 'surebet' ? 'bg-accent/10 text-accent' :
                      item.type === 'corridor' ? 'bg-blue-500/10 text-blue-400' :
                      'bg-purple-500/10 text-purple-400'
                    }`}>
                      <TypeIcon size={14} />
                    </div>
                    <span className="text-sm text-text-secondary">{typeLabels[item.type]}</span>
                  </div>
                </div>

                {/* Match */}
                <div className="col-span-3">
                  <div className="text-sm font-medium text-text-primary">{item.match}</div>
                </div>

                {/* Bookmakers */}
                <div className="col-span-2">
                  <div className="flex gap-1 flex-wrap">
                    {item.bookmakers.map((bk, i) => (
                      <span key={i} className="text-xs px-1.5 py-0.5 rounded bg-background text-text-secondary">
                        {bk}
                      </span>
                    ))}
                  </div>
                </div>

                {/* Amount */}
                <div className="col-span-2 text-right text-sm text-text-primary">
                  {formatMoney(item.amount)}
                </div>

                {/* Result */}
                <div className="col-span-2 text-right">
                  <span className={`text-sm font-medium ${
                    item.status === 'win' ? 'text-emerald-400' :
                    item.status === 'loss' ? 'text-red-400' :
                    'text-text-secondary'
                  }`}>
                    {item.status === 'win' ? '+' : item.status === 'loss' ? '-' : ''}
                    {formatMoney(Math.abs(item.profit))}
                  </span>
                </div>

                {/* Date */}
                <div className="col-span-1 text-right text-xs text-text-muted">
                  {item.date.split(' ')[0]}
                </div>
              </motion.div>
            )
          })}
        </div>

        {filtered.length === 0 && (
          <div className="py-16 text-center">
            <History size={48} className="mx-auto text-text-muted opacity-20 mb-4" />
            <div className="text-text-secondary">Нет записей</div>
          </div>
        )}
      </motion.div>
    </motion.div>
  )
}
