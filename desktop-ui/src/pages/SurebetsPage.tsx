import { useState } from 'react'
import { motion } from 'framer-motion'
import { 
  Zap, Flame, Search, Filter, ChevronRight, Timer,
  TrendingUp, TrendingDown, Star, Copy, Target,
  X, SlidersHorizontal
} from 'lucide-react'
import { demoForks } from '../lib/demoData'

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

const formatMoney = (amount: number) => new Intl.NumberFormat('ru-RU').format(amount) + ' ₽'

const sports = ['Все виды', 'Футбол', 'Теннис', 'Хоккей', 'Баскетбол']
const bookmakers = ['Все БК', 'Pari', 'Fonbet', 'Leon', 'Winline']
const profitRanges = ['Все', '>1%', '>3%', '>5%', '>10%']
const timeFilters = ['Все', 'След. час', 'Сегодня', 'Завтра']

export function SurebetsPage() {
  const [search, setSearch] = useState('')
  const [selectedSport, setSelectedSport] = useState('Все виды')
  const [selectedBookmaker, setSelectedBookmaker] = useState('Все БК')
  const [selectedProfit, setSelectedProfit] = useState('Все')
  const [selectedTime, setSelectedTime] = useState('Все')
  const [showBonusOnly, setShowBonusOnly] = useState(false)
  const [hotOnly, setHotOnly] = useState(false)

  let filtered = demoForks

  if (search) {
    filtered = filtered.filter(f => 
      f.match.toLowerCase().includes(search.toLowerCase()) ||
      f.league.toLowerCase().includes(search.toLowerCase())
    )
  }
  if (selectedSport !== 'Все виды') {
    filtered = filtered.filter(f => f.sport === selectedSport)
  }
  if (selectedBookmaker !== 'Все БК') {
    filtered = filtered.filter(f => f.bookmakers.some(b => b.name === selectedBookmaker))
  }
  if (selectedProfit !== 'Все') {
    const min = parseFloat(selectedProfit.replace('>', '').replace('%', ''))
    filtered = filtered.filter(f => f.profit >= min)
  }
  if (showBonusOnly) {
    filtered = filtered.filter(f => f.bonus)
  }
  if (hotOnly) {
    filtered = filtered.filter(f => f.isHot)
  }

  const profitColor = (profit: number) => {
    if (profit >= 5) return 'text-emerald-400 bg-emerald-500/10 border-emerald-500/30'
    if (profit >= 3) return 'text-blue-400 bg-blue-500/10 border-blue-500/30'
    if (profit >= 1) return 'text-amber-400 bg-amber-500/10 border-amber-500/30'
    return 'text-text-secondary bg-background border-border'
  }

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 space-y-6">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">Арбитражные вилки</h1>
          <p className="text-sm text-text-secondary mt-1">{filtered.length} найдено • {demoForks.filter(f => f.isHot).length} горячих</p>
        </div>
        <div className="flex gap-3">
          <button 
            className={`btn text-sm flex items-center gap-2 ${showBonusOnly ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => setShowBonusOnly(!showBonusOnly)}
          >
            <Star size={16} /> С бонусами
          </button>
          <button className="btn btn-secondary text-sm flex items-center gap-2">
            <Zap size={16} /> Обновить
          </button>
        </div>
      </motion.div>

      {/* Filters */}
      <motion.div variants={item} className="flex flex-wrap items-center gap-3">
        <div className="relative">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-text-muted" />
          <input 
            type="text" 
            placeholder="Поиск команд, лиг..."
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="input pl-9 w-64"
          />
        </div>
        
        <select value={selectedSport} onChange={e => setSelectedSport(e.target.value)} className="input w-32">
          {sports.map(s => <option key={s}>{s}</option>)}
        </select>
        
        <select value={selectedBookmaker} onChange={e => setSelectedBookmaker(e.target.value)} className="input w-32">
          {bookmakers.map(b => <option key={b}>{b}</option>)}
        </select>
        
        <select value={selectedProfit} onChange={e => setSelectedProfit(e.target.value)} className="input w-28">
          {profitRanges.map(p => <option key={p}>{p}</option>)}
        </select>
        
        <select value={selectedTime} onChange={e => setSelectedTime(e.target.value)} className="input w-32">
          {timeFilters.map(t => <option key={t}>{t}</option>)}
        </select>

        <button 
          className={`btn text-sm flex items-center gap-2 ${hotOnly ? 'btn-primary' : 'btn-secondary'}`}
          onClick={() => setHotOnly(!hotOnly)}
        >
          <Flame size={16} /> Горячие
        </button>
      </motion.div>

      {/* Forks Table */}
      <motion.div variants={item} className="rounded-card border border-border bg-surface overflow-hidden">
        {/* Table Header */}
        <div className="grid grid-cols-12 gap-4 px-5 py-3 bg-background border-b border-border text-xs text-text-secondary uppercase tracking-wider">
          <div className="col-span-3">Матч</div>
          <div className="col-span-2">Рынок</div>
          <div className="col-span-1 text-center">Профит</div>
          <div className="col-span-2">БК 1</div>
          <div className="col-span-2">БК 2</div>
          <div className="col-span-1 text-center">Сумма</div>
          <div className="col-span-1 text-center">Время</div>
        </div>

        {/* Table Body */}
        <div className="divide-y divide-border/50">
          {filtered.map((fork) => (
            <motion.div 
              key={fork.id}
              className="grid grid-cols-12 gap-4 px-5 py-4 items-center hover:bg-elevated/30 transition-colors cursor-pointer group"
              whileHover={{ x: 2 }}
            >
              {/* Match */}
              <div className="col-span-3">
                <div className="flex items-center gap-2">
                  {fork.isHot && <Flame size={14} className="text-orange-400 shrink-0" />}
                  <div>
                    <div className="text-sm font-medium text-text-primary">{fork.match}</div>
                    <div className="text-xs text-text-secondary">{fork.league}</div>
                  </div>
                </div>
              </div>

              {/* Market */}
              <div className="col-span-2 text-sm text-text-secondary">{fork.market}</div>

              {/* Profit */}
              <div className="col-span-1 text-center">
                <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-bold border ${profitColor(fork.profit)}`}>
                  +{fork.profit}%
                  {fork.bonus && <Star size={10} className="ml-1 text-purple-400" />}
                </span>
              </div>

              {/* Bookmaker 1 */}
              <div className="col-span-2">
                <div className="text-sm text-text-primary">{fork.bookmakers[0].name}</div>
                <div className="text-xs text-text-secondary">{fork.bookmakers[0].outcome} @ {fork.bookmakers[0].odds}</div>
              </div>

              {/* Bookmaker 2 */}
              <div className="col-span-2">
                <div className="text-sm text-text-primary">{fork.bookmakers[1].name}</div>
                <div className="text-xs text-text-secondary">{fork.bookmakers[1].outcome} @ {fork.bookmakers[1].odds}</div>
              </div>

              {/* Total Stake */}
              <div className="col-span-1 text-center text-sm font-medium text-text-primary">{formatMoney(fork.totalStake)}</div>

              {/* Time */}
              <div className="col-span-1 text-center">
                <div className={`text-xs flex items-center justify-center gap-1 ${fork.timeLeft < 600 ? 'text-red-400' : 'text-text-secondary'}`}>
                  <Timer size={12} />
                  {Math.floor(fork.timeLeft / 60)}:{(fork.timeLeft % 60).toString().padStart(2, '0')}
                </div>
              </div>
            </motion.div>
          ))}
        </div>

        {filtered.length === 0 && (
          <div className="py-16 text-center">
            <Search size={48} className="mx-auto text-text-muted opacity-20 mb-4" />
            <div className="text-text-secondary">Вилки не найдены</div>
            <div className="text-sm text-text-muted mt-1">Попробуйте изменить фильтры</div>
          </div>
        )}
      </motion.div>
    </motion.div>
  )
}
