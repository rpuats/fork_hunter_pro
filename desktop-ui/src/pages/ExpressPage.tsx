import { useState } from 'react'
import { motion } from 'framer-motion'
import { 
  Layers, Plus, Trash2, Calculator, TrendingUp, AlertTriangle,
  ChevronRight, Save, Zap, Target, Percent, DollarSign
} from 'lucide-react'
import { StatCard } from '../components/StatCard'

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

const demoExpressForks = [
  {
    id: 'e1',
    name: 'Футбол на выходных',
    events: [
      { match: 'ЦСКА — Спартак', outcome: 'П1', odds: 2.45 },
      { match: 'Зенит — Локомотив', outcome: 'ТБ 2.5', odds: 1.85 },
      { match: 'Динамо — Краснодар', outcome: 'Х', odds: 3.20 }
    ],
    totalOdds: 14.52,
    stake: 5000,
    potentialWin: 72600,
    probability: 12,
    bookmaker: 'Pari'
  },
  {
    id: 'e2',
    name: 'Теннис на сегодня',
    events: [
      { match: 'Медведев — Сinner', outcome: 'П1', odds: 1.75 },
      { match: 'Алькарас — Джокович', outcome: 'ТБ 22.5', odds: 1.90 }
    ],
    totalOdds: 3.33,
    stake: 10000,
    potentialWin: 33300,
    probability: 35,
    bookmaker: 'Fonbet'
  }
]

const formatMoney = (amount: number) => new Intl.NumberFormat('ru-RU').format(amount) + ' ₽'

export function ExpressPage() {
  const [expresses, setExpresses] = useState(demoExpressForks)
  const [showBuilder, setShowBuilder] = useState(false)
  const [builderEvents, setBuilderEvents] = useState([{ match: '', outcome: '', odds: 1.0 }])
  const [stake, setStake] = useState(5000)

  const addEvent = () => {
    setBuilderEvents([...builderEvents, { match: '', outcome: '', odds: 1.0 }])
  }

  const removeEvent = (index: number) => {
    setBuilderEvents(builderEvents.filter((_, i) => i !== index))
  }

  const updateEvent = (index: number, field: string, value: string | number) => {
    const updated = [...builderEvents]
    updated[index] = { ...updated[index], [field]: value }
    setBuilderEvents(updated)
  }

  const totalOdds = builderEvents.reduce((acc, e) => acc * (Number(e.odds) || 1), 1)
  const potentialWin = stake * totalOdds

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 space-y-6">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">Экспресс-вилки</h1>
          <p className="text-sm text-text-secondary mt-1">Создание и расчёт экспресс-ставок</p>
        </div>
        <button 
          onClick={() => setShowBuilder(!showBuilder)}
          className="btn btn-primary text-sm flex items-center gap-2"
        >
          {showBuilder ? <Trash2 size={16} /> : <Plus size={16} />}
          {showBuilder ? 'Отменить' : 'Собрать экспресс'}
        </button>
      </motion.div>

      {/* Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <motion.div variants={item}>
          <StatCard icon={Layers} label="Экспрессов" value={expresses.length} color="blue" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={Target} label="Средний коэф." value="5.92" color="purple" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={TrendingUp} label="Потенциал" value={formatMoney(105900)} color="green" />
        </motion.div>
        <motion.div variants={item}>
          <StatCard icon={Percent} label="Средняя вероятность" value="23%" color="orange" />
        </motion.div>
      </div>

      {/* Express Builder */}
      {showBuilder && (
        <motion.div 
          variants={item}
          initial={{ opacity: 0, height: 0 }}
          animate={{ opacity: 1, height: 'auto' }}
          className="rounded-card border border-accent/30 bg-surface p-5"
        >
          <h3 className="text-base font-semibold text-text-primary mb-4 flex items-center gap-2">
            <Calculator size={18} className="text-accent" /> Конструктор экспресса
          </h3>

          <div className="space-y-3 mb-4">
            {builderEvents.map((event, index) => (
              <div key={index} className="flex gap-2 items-start">
                <div className="flex-1">
                  <input
                    type="text"
                    placeholder="Матч (например: ЦСКА — Спартак)"
                    value={event.match}
                    onChange={e => updateEvent(index, 'match', e.target.value)}
                    className="input w-full mb-2"
                  />
                  <div className="flex gap-2">
                    <input
                      type="text"
                      placeholder="Исход (П1, ТБ 2.5...)"
                      value={event.outcome}
                      onChange={e => updateEvent(index, 'outcome', e.target.value)}
                      className="input flex-1"
                    />
                    <input
                      type="number"
                      step="0.01"
                      placeholder="Коэф."
                      value={event.odds}
                      onChange={e => updateEvent(index, 'odds', parseFloat(e.target.value))}
                      className="input w-24"
                    />
                  </div>
                </div>
                <button
                  onClick={() => removeEvent(index)}
                  className="p-2 rounded-lg hover:bg-red-500/10 text-text-muted hover:text-red-400 transition-colors"
                >
                  <Trash2 size={18} />
                </button>
              </div>
            ))}
          </div>

          <button
            onClick={addEvent}
            className="w-full py-2 rounded-lg border border-dashed border-border hover:border-accent/50 text-text-muted hover:text-accent transition-colors text-sm mb-4"
          >
            + Добавить событие
          </button>

          {/* Calculation */}
          <div className="p-4 rounded-lg bg-background space-y-2">
            <div className="flex justify-between text-sm">
              <span className="text-text-secondary">Общий коэффициент:</span>
              <span className="text-text-primary font-medium">{totalOdds.toFixed(2)}</span>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-text-secondary">Сумма ставки:</span>
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  value={stake}
                  onChange={e => setStake(Number(e.target.value))}
                  className="input w-24 text-right"
                />
                <span className="text-text-secondary">₽</span>
              </div>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-text-secondary">Потенциальный выигрыш:</span>
              <span className="text-emerald-400 font-medium">{formatMoney(potentialWin)}</span>
            </div>
          </div>

          <div className="flex gap-3 mt-4">
            <button className="btn btn-secondary flex-1 text-sm flex items-center justify-center gap-2">
              <Save size={16} /> Сохранить
            </button>
            <button className="btn btn-primary flex-1 text-sm flex items-center justify-center gap-2">
              <Zap size={16} /> Разместить
            </button>
          </div>
        </motion.div>
      )}

      {/* Saved Expresses */}
      <motion.div variants={item}>
        <h3 className="text-base font-semibold text-text-primary mb-4">Сохранённые экспрессы</h3>
        <div className="space-y-3">
          {expresses.map(express => (
            <motion.div
              key={express.id}
              className="rounded-card border border-border bg-surface p-4 hover:border-accent/30 transition-all cursor-pointer"
              whileHover={{ x: 4 }}
            >
              <div className="flex items-start justify-between mb-3">
                <div>
                  <h4 className="font-semibold text-text-primary">{express.name}</h4>
                  <div className="text-xs text-text-secondary">{express.events.length} событий • {express.bookmaker}</div>
                </div>
                <div className="text-right">
                  <div className="text-lg font-bold text-accent">{express.totalOdds.toFixed(2)}</div>
                  <div className="text-xs text-text-secondary">коэффициент</div>
                </div>
              </div>

              {/* Events list */}
              <div className="space-y-2 mb-4">
                {express.events.map((event, i) => (
                  <div key={i} className="flex items-center gap-3 text-sm">
                    <div className="w-6 h-6 rounded-full bg-background flex items-center justify-center text-xs text-text-muted">
                      {i + 1}
                    </div>
                    <div className="flex-1 text-text-secondary">{event.match}</div>
                    <div className="text-text-primary">{event.outcome}</div>
                    <div className="text-accent">@{event.odds}</div>
                  </div>
                ))}
              </div>

              {/* Stats */}
              <div className="flex items-center gap-4 pt-3 border-t border-border">
                <div className="text-sm">
                  <span className="text-text-secondary">Ставка: </span>
                  <span className="text-text-primary">{formatMoney(express.stake)}</span>
                </div>
                <div className="text-sm">
                  <span className="text-text-secondary">Потенциал: </span>
                  <span className="text-emerald-400">{formatMoney(express.potentialWin)}</span>
                </div>
                <div className="text-sm">
                  <span className="text-text-secondary">Вероятность: </span>
                  <span className="text-text-primary">{express.probability}%</span>
                </div>
                <button className="ml-auto btn btn-secondary text-xs flex items-center gap-1">
                  Разместить <ChevronRight size={14} />
                </button>
              </div>
            </motion.div>
          ))}
        </div>
      </motion.div>
    </motion.div>
  )
}
