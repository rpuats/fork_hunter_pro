import { useState } from 'react'
import { motion } from 'framer-motion'
import { 
  GitBranch, TrendingUp, ArrowRight, Percent, Target,
  Filter, Search, Zap, ChevronRight, AlertTriangle,
  BarChart3, Calendar, Trophy, Flame
} from 'lucide-react'
import { StatCard } from '../components/StatCard'

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

const demoCorridors = [
  {
    id: 'c1',
    match: 'ЦСКА — Спартак',
    league: 'РПЛ',
    sport: 'Футбол',
    type: 'Тотал',
    bet1: { bookmaker: 'Pari', outcome: 'ТБ 2.5', odds: 1.85 },
    bet2: { bookmaker: 'Fonbet', outcome: 'ТМ 3.5', odds: 1.75 },
    roi: 12.5,
    probability: 78,
    stake: 10000,
    potentialProfit: 1250,
    timeLeft: 3600,
    isHot: true
  },
  {
    id: 'c2',
    match: 'Реал Мадрид — Барселона',
    league: 'Ла Лига',
    sport: 'Футбол',
    type: 'Фора',
    bet1: { bookmaker: 'Leon', outcome: 'Ф1 +0.5', odds: 1.90 },
    bet2: { bookmaker: 'Winline', outcome: 'Ф2 +0.5', odds: 1.95 },
    roi: 8.2,
    probability: 65,
    stake: 15000,
    potentialProfit: 1230,
    timeLeft: 7200,
    isHot: false
  },
  {
    id: 'c3',
    match: 'Локомотив — Зенит',
    league: 'РПЛ',
    sport: 'Футбол',
    type: 'ИТотал',
    bet1: { bookmaker: 'Pari', outcome: 'ИТБ1 1.5', odds: 1.70 },
    bet2: { bookmaker: 'Leon', outcome: 'ИТМ1 2.5', odds: 2.10 },
    roi: 15.3,
    probability: 85,
    stake: 8000,
    potentialProfit: 1224,
    timeLeft: 1800,
    isHot: true
  },
  {
    id: 'c4',
    match: 'Манчестер Сити — Ливерпуль',
    league: 'АПЛ',
    sport: 'Футбол',
    type: 'Тотал',
    bet1: { bookmaker: 'Fonbet', outcome: 'ТБ 2.5', odds: 1.80 },
    bet2: { bookmaker: 'Winline', outcome: 'ТМ 3.5', odds: 1.80 },
    roi: 6.8,
    probability: 58,
    stake: 12000,
    potentialProfit: 816,
    timeLeft: 5400,
    isHot: false
  }
]

const formatMoney = (amount: number) => new Intl.NumberFormat('ru-RU').format(amount) + ' ₽'

export function CorridorsPage() {
  const [search, setSearch] = useState('')
  const [minRoi, setMinRoi] = useState(0)
  const [showHotOnly, setShowHotOnly] = useState(false)

  const filtered = demoCorridors.filter(c => {
    if (search && !c.match.toLowerCase().includes(search.toLowerCase())) return false
    if (c.roi < minRoi) return false
    if (showHotOnly && !c.isHot) return false
    return true
  })

  const totalPotential = filtered.reduce((sum, c) => sum + c.potentialProfit, 0)
  const avgRoi = filtered.length > 0 ? filtered.reduce((sum, c) => sum + c.roi, 0) / filtered.length : 0

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 space-y-6">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">Коридоры</h1>
          <p className="text-sm text-text-secondary mt-1">
            {filtered.length} найдено • Потенциал: {formatMoney(totalPotential)} • Средний ROI: {avgRoi.toFixed(1)}%
          </p>
        </div>
        <button className="btn btn-secondary text-sm flex items-center gap-2">
          <Zap size={16} /> Обновить
        </button>
      </motion.div>

      {/* Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div variants={item}>
          <StatCard icon={GitBranch} label="Коридоров" value={demoCorridors.length} color="blue" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={TrendingUp} label="Средний ROI" value={`${avgRoi.toFixed(1)}%`} color="green" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={Target} label="Потенциал" value={formatMoney(totalPotential)} color="purple" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={Flame} label="Горячих" value={demoCorridors.filter(c => c.isHot).length} color="orange" />
        </motion.div>
      </div>

      {/* Filters */}
      <motion.div variants={item} className="flex flex-wrap items-center gap-3">
        <div className="relative">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted" />
          <input 
            type="text" 
            placeholder="Поиск матчей..."
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="input pl-9 w-64"
          />
        </div>
        
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-surface border border-border">
          <TrendingUp size={14} className="text-text-muted" />
          <span className="text-sm text-text-secondary">ROI &gt;</span>
          <input 
            type="number" 
            value={minRoi}
            onChange={e => setMinRoi(Number(e.target.value))}
            className="w-16 bg-transparent text-sm text-text-primary outline-none"
          />
          <span className="text-sm text-text-secondary">%</span>
        </div>

        <button 
          onClick={() => setShowHotOnly(!showHotOnly)}
          className={`btn text-sm flex items-center gap-2 ${showHotOnly ? 'btn-primary' : 'btn-secondary'}`}
        >
          <Flame size={14} /> Горячие
        </button>
      </motion.div>

      {/* Corridors Grid */}
      <motion.div variants={item} className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {filtered.map(corridor => (
          <motion.div 
            key={corridor.id}
            className="rounded-card border border-border bg-surface p-5 hover:border-accent/30 transition-all cursor-pointer group"
            whileHover={{ y: -2 }}
          >
            {/* Header */}
            <div className="flex items-start justify-between mb-4">
              <div>
                <div className="flex items-center gap-2 mb-1">
                  {corridor.isHot && <Flame size={14} className="text-orange-400" />}
                  <h3 className="font-semibold text-text-primary">{corridor.match}</h3>
                </div>
                <div className="text-xs text-text-secondary">{corridor.league} • {corridor.type}</div>
              </div>
              <div className={`text-sm font-bold px-2 py-1 rounded-lg ${
                corridor.roi >= 12 ? 'bg-emerald-500/10 text-emerald-400' :
                corridor.roi >= 8 ? 'bg-blue-500/10 text-blue-400' :
                'bg-amber-500/10 text-amber-400'
              }`}>
                +{corridor.roi}% ROI
              </div>
            </div>

            {/* Bets visualization */}
            <div className="flex items-center gap-3 mb-4">
              {/* Bet 1 */}
              <div className="flex-1 p-3 rounded-lg bg-background">
                <div className="text-xs text-text-muted mb-1">{corridor.bet1.bookmaker}</div>
                <div className="text-sm font-medium text-text-primary">{corridor.bet1.outcome}</div>
                <div className="text-xs text-accent">@{corridor.bet1.odds}</div>
              </div>

              {/* Arrow */}
              <div className="flex flex-col items-center">
                <ArrowRight size={16} className="text-text-muted" />
                <span className="text-[10px] text-text-muted">или</span>
              </div>

              {/* Bet 2 */}
              <div className="flex-1 p-3 rounded-lg bg-background">
                <div className="text-xs text-text-muted mb-1">{corridor.bet2.bookmaker}</div>
                <div className="text-sm font-medium text-text-primary">{corridor.bet2.outcome}</div>
                <div className="text-xs text-accent">@{corridor.bet2.odds}</div>
              </div>
            </div>

            {/* Stats */}
            <div className="grid grid-cols-3 gap-3 mb-4">
              <div className="text-center p-2 rounded bg-background">
                <div className="text-sm font-medium text-text-primary">{corridor.probability}%</div>
                <div className="text-[10px] text-text-muted">Вероятность</div>
              </div>
              <div className="text-center p-2 rounded bg-background">
                <div className="text-sm font-medium text-text-primary">{formatMoney(corridor.stake)}</div>
                <div className="text-[10px] text-text-muted">Ставка</div>
              </div>
              <div className="text-center p-2 rounded bg-background">
                <div className="text-sm font-medium text-emerald-400">{formatMoney(corridor.potentialProfit)}</div>
                <div className="text-[10px] text-text-muted">Профит</div>
              </div>
            </div>

            {/* Action */}
            <button className="w-full btn btn-secondary text-sm flex items-center justify-center gap-2">
              Разместить ставки <ChevronRight size={14} />
            </button>
          </motion.div>
        ))}
      </motion.div>

      {filtered.length === 0 && (
        <motion.div variants={item} className="text-center py-16">
          <GitBranch size={48} className="mx-auto text-text-muted opacity-20 mb-4" />
          <p className="text-text-secondary">Коридоры не найдены</p>
          <p className="text-sm text-text-muted mt-1">Попробуйте изменить фильтры</p>
        </motion.div>
      )}
    </motion.div>
  )
}
