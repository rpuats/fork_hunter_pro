import { useState, useMemo } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Search, Filter, ArrowUpDown, ExternalLink, Clock, Zap, Target, X, Calculator, ChevronDown, ChevronUp } from 'lucide-react'
import { toast } from 'sonner'
import type { Surebet } from '../types'

interface SurebetsPageProps {
  surebets: Surebet[]
}

export function SurebetsPage({ surebets }: SurebetsPageProps) {
  const [search, setSearch] = useState('')
  const [minProfit, setMinProfit] = useState(0)
  const [marketFilter, setMarketFilter] = useState('all')
  const [sortBy, setSortBy] = useState<'profit' | 'time'>('profit')
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [showDetails, setShowDetails] = useState<Surebet | null>(null)

  const filtered = useMemo(() => {
    let result = [...surebets]

    if (search) {
      const s = search.toLowerCase()
      result = result.filter(sb =>
        sb.home_team.toLowerCase().includes(s) ||
        sb.away_team.toLowerCase().includes(s) ||
        sb.league.toLowerCase().includes(s) ||
        sb.legs.some(l => l.bookmaker.toLowerCase().includes(s))
      )
    }

    if (minProfit > 0) {
      result = result.filter(sb => sb.profit_percent >= minProfit)
    }

    if (marketFilter !== 'all') {
      result = result.filter(sb =>
        sb.legs.some(leg => leg.market.toLowerCase().includes(marketFilter))
      )
    }

    result.sort((a, b) => {
      if (sortBy === 'profit') return b.profit_percent - a.profit_percent
      return new Date(b.detected_at).getTime() - new Date(a.detected_at).getTime()
    })

    return result.slice(0, 100)
  }, [surebets, search, minProfit, marketFilter, sortBy])

  const markets = ['all', '1x2', 'total', 'handicap', 'btts']

  const handleCopyStakes = (sb: Surebet) => {
    const text = sb.legs.map(l => 
      `${l.bookmaker}: ${l.selection} @ ${l.odds.toFixed(2)} → ${l.stake.toFixed(0)}₽`
    ).join('\n')
    navigator.clipboard.writeText(text)
    toast.success('Ставки скопированы в буфер обмена')
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Арбитражные вилки</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            {filtered.length} вилок найдено • {surebets.length} всего
          </p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            <Zap size={14} style={{ color: 'var(--accent-green)' }} />
            <span className="text-xs font-medium">Real-time</span>
          </div>
        </div>
      </div>

      {/* Filters */}
      <motion.div 
        className="glass-card p-4"
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
      >
        <div className="flex flex-wrap gap-3 items-center">
          <div className="flex items-center gap-2 flex-1 min-w-[240px]">
            <Search size={16} style={{ color: 'var(--text-muted)', flexShrink: 0 }} />
            <input
              type="text"
              placeholder="Поиск команд, лиг, БК..."
              value={search}
              onChange={e => setSearch(e.target.value)}
              className="input flex-1"
            />
          </div>

          <div className="flex items-center gap-2">
            <Filter size={14} style={{ color: 'var(--text-muted)' }} />
            <select
              value={marketFilter}
              onChange={e => setMarketFilter(e.target.value)}
              className="input !w-auto !py-2"
            >
              {markets.map(m => (
                <option key={m} value={m}>
                  {m === 'all' ? 'Все рынки' : m.toUpperCase()}
                </option>
              ))}
            </select>
          </div>

          <div className="flex items-center gap-2">
            <Target size={14} style={{ color: 'var(--text-muted)' }} />
            <input
              type="number"
              placeholder="Мин %"
              value={minProfit || ''}
              onChange={e => setMinProfit(parseFloat(e.target.value) || 0)}
              className="input !w-24 !py-2"
              step="0.1"
              min="0"
            />
          </div>

          <button
            onClick={() => setSortBy(sortBy === 'profit' ? 'time' : 'profit')}
            className="btn btn-ghost"
          >
            <ArrowUpDown size={14} />
            {sortBy === 'profit' ? 'По прибыли' : 'По времени'}
          </button>
        </div>
      </motion.div>

      {/* Table */}
      <motion.div 
        className="table-container"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.1 }}
      >
        <table className="w-full">
          <thead>
            <tr>
              <th className="table-header">Матч</th>
              <th className="table-header">Рынок</th>
              <th className="table-header">Прибыль</th>
              <th className="table-header">БК 1</th>
              <th className="table-header">БК 2</th>
              <th className="table-header">Объём</th>
              <th className="table-header">Время</th>
              <th className="table-header"></th>
            </tr>
          </thead>
          <tbody>
            <AnimatePresence>
              {filtered.map((sb, index) => (
                <motion.tr 
                  key={sb.id} 
                  className="table-row surebet-row"
                  initial={{ opacity: 0, x: -20 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: 20 }}
                  transition={{ delay: index * 0.02 }}
                  onClick={() => setExpandedId(expandedId === sb.id ? null : sb.id)}
                >
                  <td className="table-cell">
                    <div>
                      <p className="font-medium">{sb.home_team}</p>
                      <p className="text-xs mt-0.5" style={{ color: 'var(--text-secondary)' }}>{sb.away_team}</p>
                    </div>
                  </td>
                  <td className="table-cell">
                    <span className="badge badge-info">{sb.legs[0]?.market || '—'}</span>
                  </td>
                  <td className="table-cell">
                    <span className="profit profit-positive text-lg">+{sb.profit_percent.toFixed(2)}%</span>
                  </td>
                  <td className="table-cell">
                    <div>
                      <p className="text-sm font-medium capitalize">{sb.legs[0]?.bookmaker}</p>
                      <p className="text-xs mt-0.5" style={{ color: 'var(--text-secondary)' }}>
                        {sb.legs[0]?.selection} <span className="font-mono">@ {sb.legs[0]?.odds.toFixed(2)}</span>
                      </p>
                    </div>
                  </td>
                  <td className="table-cell">
                    <div>
                      <p className="text-sm font-medium capitalize">{sb.legs[1]?.bookmaker || '—'}</p>
                      <p className="text-xs mt-0.5" style={{ color: 'var(--text-secondary)' }}>
                        {sb.legs[1]?.selection} <span className="font-mono">@ {sb.legs[1]?.odds.toFixed(2)}</span>
                      </p>
                    </div>
                  </td>
                  <td className="table-cell">
                    <span className="text-sm font-mono">{sb.total_stake.toLocaleString()}₽</span>
                  </td>
                  <td className="table-cell">
                    <div className="flex items-center gap-1" style={{ color: 'var(--text-muted)' }}>
                      <Clock size={12} />
                      <span className="text-xs">{new Date(sb.detected_at).toLocaleTimeString('ru-RU')}</span>
                    </div>
                  </td>
                  <td className="table-cell">
                    <div className="flex items-center gap-1">
                      <button
                        onClick={(e) => { e.stopPropagation(); setShowDetails(sb) }}
                        className="btn btn-ghost !px-2 !py-1.5"
                      >
                        <ExternalLink size={14} />
                      </button>
                    </div>
                  </td>
                </motion.tr>
              ))}
            </AnimatePresence>
            
            {filtered.length === 0 && (
              <tr>
                <td colSpan={8} className="text-center py-16" style={{ color: 'var(--text-muted)' }}>
                  <Zap size={48} className="mx-auto mb-3 opacity-20" />
                  <p className="text-base">Вилок не найдено</p>
                  <p className="text-sm mt-1">Попробуйте изменить фильтры или подождите</p>
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </motion.div>

      {/* Details Modal */}
      <AnimatePresence>
        {showDetails && (
          <motion.div 
            className="fixed inset-0 z-50 flex items-center justify-center p-4"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setShowDetails(null)}
          >
            <div className="absolute inset-0" style={{ background: 'rgba(0,0,0,0.7)', backdropFilter: 'blur(4px)' }} />
            
            <motion.div 
              className="relative w-full max-w-lg rounded-2xl p-6"
              style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}
              initial={{ scale: 0.9, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.9, opacity: 0 }}
              onClick={e => e.stopPropagation()}
            >
              <button 
                onClick={() => setShowDetails(null)}
                className="absolute top-4 right-4 p-1 rounded-lg transition-colors"
                style={{ color: 'var(--text-muted)' }}
              >
                <X size={18} />
              </button>

              <h3 className="text-lg font-bold mb-1">{showDetails.home_team} — {showDetails.away_team}</h3>
              <p className="text-sm mb-4" style={{ color: 'var(--text-secondary)' }}>{showDetails.league}</p>

              <div className="flex items-center justify-between p-4 rounded-xl mb-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div>
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Прибыль</p>
                  <p className="profit profit-positive text-2xl">+{showDetails.profit_percent.toFixed(2)}%</p>
                </div>
                <div className="text-right">
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Общий объём</p>
                  <p className="text-xl font-mono">{showDetails.total_stake.toLocaleString()}₽</p>
                </div>
              </div>

              <div className="space-y-3 mb-4">
                {showDetails.legs.map((leg, i) => (
                  <div key={i} className="flex items-center justify-between p-3 rounded-lg" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                    <div>
                      <p className="text-sm font-medium capitalize">{leg.bookmaker}</p>
                      <p className="text-xs mt-0.5" style={{ color: 'var(--text-secondary)' }}>
                        {leg.selection} @ <span className="font-mono">{leg.odds.toFixed(2)}</span>
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="text-sm font-mono">{leg.stake.toFixed(0)}₽</p>
                      <p className="text-xs" style={{ color: 'var(--text-muted)' }}>→ {leg.payout.toFixed(0)}₽</p>
                    </div>
                  </div>
                ))}
              </div>

              <div className="flex gap-2">
                <button 
                  onClick={() => handleCopyStakes(showDetails)}
                  className="btn btn-primary flex-1"
                >
                  <Calculator size={16} />
                  Копировать ставки
                </button>
                <button 
                  onClick={() => { toast.info('Переход на БК...'); setShowDetails(null) }}
                  className="btn btn-ghost"
                >
                  <ExternalLink size={16} />
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
