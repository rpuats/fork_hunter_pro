import { useState } from 'react'
import { motion } from 'framer-motion'
import { 
  MousePointer, Bot, Zap, Check, AlertTriangle, Settings,
  SlidersHorizontal, Bell, TrendingUp, XCircle, Clock,
  Target, DollarSign, Timer, Shield, ChevronRight, Play,
  Pause, RotateCcw, Filter
} from 'lucide-react'

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

type AutoBetMode = 'manual' | 'semi_auto' | 'full_auto'

interface ModeOption {
  id: AutoBetMode
  icon: any
  title: string
  description: string
  pros: string[]
  cons: string[]
  recommended?: boolean
  warning?: boolean
}

const modes: ModeOption[] = [
  {
    id: 'manual',
    icon: MousePointer,
    title: 'Ручной',
    description: 'Вы сами принимаете решения по каждой вилке',
    pros: ['Полный контроль', 'Нет риска ошибок'],
    cons: ['Требует времени', 'Можно упустить вилки']
  },
  {
    id: 'semi_auto',
    icon: Bot,
    title: 'Полуавто',
    description: 'Система готовит купон, вы подтверждаете',
    pros: ['Быстрее ручного', 'Контроль за ставками'],
    cons: ['Нужно быть онлайн', 'Задержка на подтверждение'],
    recommended: true
  },
  {
    id: 'full_auto',
    icon: Zap,
    title: 'Полное авто',
    description: 'Система ставит самостоятельно',
    pros: ['Максимальная скорость', 'Не нужно быть онлайн'],
    cons: ['Высокий риск', 'Требует настройки'],
    warning: true
  }
]

const demoStats = {
  betsToday: 12,
  wins: 8,
  losses: 4,
  profit: 3450,
  winRate: 67,
  avgProfit: 3.2,
  bestFork: 7.1,
  errors: 1
}

export function OperatorPage() {
  const [mode, setMode] = useState<AutoBetMode>('semi_auto')
  const [isEnabled, setIsEnabled] = useState(false)
  const [settings, setSettings] = useState({
    minStake: 1000,
    maxStake: 25000,
    maxDailyLoss: 50000,
    minProfit: 1.5,
    maxBetsPerDay: 50,
    cooldownAfterBet: 5,
    cooldownAfterLoss: 15,
    maxConcurrentBets: 3,
    requireConfirmation: true,
    confirmationTimeout: 30,
    stopAfterLosses: true,
    stopAfterLossesCount: 3,
    notifyOnBet: true,
    notifyOnWin: true,
    notifyOnLoss: true,
    notifyOnError: true
  })

  const pendingBets = [
    { id: 'p1', match: 'ЦСКА — Спартак', profit: 5.2, stake: 10000, bookmakers: ['Pari', 'Fonbet'], timeLeft: 45 },
    { id: 'p2', match: 'Реал Мадрид — Барселона', profit: 7.1, stake: 15000, bookmakers: ['Fonbet', 'Leon'], timeLeft: 120 }
  ]

  const actionLogs = [
    { id: 'l1', action: 'Ставка размещена', detail: 'ЦСКА — Спартак @ Pari', time: '2 мин назад', type: 'success' },
    { id: 'l2', action: 'Вилка найдена', detail: 'Реал Мадрид — Барселона 7.1%', time: '5 мин назад', type: 'info' },
    { id: 'l3', action: 'Ошибка', detail: 'Таймаут Leon API', time: '12 мин назад', type: 'error' },
    { id: 'l4', action: 'Ставка выиграна', detail: 'Зенит — Локомотив +1,200 ₽', time: '1 час назад', type: 'win' }
  ]

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 space-y-6 max-w-5xl mx-auto">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">Авто-ставки</h1>
          <p className="text-sm text-text-secondary mt-1">Автоматическое размещение ставок на вилки</p>
        </div>
        <div className="flex items-center gap-3">
          <div className={`px-3 py-1.5 rounded-full text-sm font-medium ${isEnabled ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30' : 'bg-gray-500/20 text-gray-400 border border-gray-500/30'}`}>
            {isEnabled ? 'Активно' : 'Выключено'}
          </div>
          <button 
            onClick={() => setIsEnabled(!isEnabled)}
            className={`w-14 h-7 rounded-full transition-colors relative ${isEnabled ? 'bg-emerald-500' : 'bg-gray-600'}`}
          >
            <motion.div 
              className="w-5 h-5 rounded-full bg-white absolute top-1"
              animate={{ left: isEnabled ? 32 : 4 }}
            />
          </button>
        </div>
      </motion.div>

      {/* Mode Selection */}
      <motion.div variants={item} className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {modes.map((m) => {
          const Icon = m.icon
          const isActive = mode === m.id
          return (
            <button
              key={m.id}
              onClick={() => setMode(m.id)}
              className={`relative rounded-card border p-5 text-left transition-all ${
                isActive 
                  ? 'border-accent bg-accent/5' 
                  : 'border-border bg-surface hover:border-accent/30'
              }`}
            >
              {m.recommended && (
                <span className="absolute -top-2 left-4 px-2 py-0.5 rounded-full bg-accent text-white text-[10px] font-medium">
                  Рекомендуем
                </span>
              )}
              {m.warning && (
                <span className="absolute -top-2 left-4 px-2 py-0.5 rounded-full bg-red-500 text-white text-[10px] font-medium">
                  Осторожно
                </span>
              )}
              
              <div className="flex items-center gap-3 mb-3">
                <div className={`p-2 rounded-lg ${isActive ? 'bg-accent/20 text-accent' : 'bg-background text-text-secondary'}`}>
                  <Icon size={24} />
                </div>
                <div>
                  <h3 className={`font-semibold ${isActive ? 'text-accent' : 'text-text-primary'}`}>{m.title}</h3>
                </div>
                {isActive && <Check size={18} className="text-accent ml-auto" />}
              </div>
              
              <p className="text-sm text-text-secondary mb-3">{m.description}</p>
              
              <div className="space-y-1">
                {m.pros.map((pro, i) => (
                  <div key={i} className="flex items-center gap-1.5 text-xs text-emerald-400">
                    <Check size={12} /> {pro}
                  </div>
                ))}
                {m.cons.map((con, i) => (
                  <div key={i} className="flex items-center gap-1.5 text-xs text-text-muted">
                    <XCircle size={12} /> {con}
                  </div>
                ))}
              </div>
            </button>
          )
        })}
      </motion.div>

      {/* Settings */}
      <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
        <div className="flex items-center gap-2 mb-5">
          <Settings size={18} className="text-accent" />
          <h3 className="text-base font-semibold text-text-primary">Настройки {mode === 'manual' ? 'фильтров' : 'авто-ставок'}</h3>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-5">
          {/* Financial Limits */}
          <div className="space-y-4">
            <h4 className="text-sm font-medium text-text-secondary flex items-center gap-2">
              <DollarSign size={14} /> Финансовые лимиты
            </h4>
            
            <div>
              <label className="text-xs text-text-secondary mb-1 block">Мин. ставка</label>
              <div className="flex items-center gap-2">
                <input 
                  type="number" 
                  value={settings.minStake}
                  onChange={e => setSettings({...settings, minStake: Number(e.target.value)})}
                  className="input flex-1"
                />
                <span className="text-sm text-text-secondary">₽</span>
              </div>
            </div>

            <div>
              <label className="text-xs text-text-secondary mb-1 block">Макс. ставка</label>
              <div className="flex items-center gap-2">
                <input 
                  type="number" 
                  value={settings.maxStake}
                  onChange={e => setSettings({...settings, maxStake: Number(e.target.value)})}
                  className="input flex-1"
                />
                <span className="text-sm text-text-secondary">₽</span>
              </div>
            </div>

            <div>
              <label className="text-xs text-text-secondary mb-1 block">Макс. проигрыш в день</label>
              <div className="flex items-center gap-2">
                <input 
                  type="number" 
                  value={settings.maxDailyLoss}
                  onChange={e => setSettings({...settings, maxDailyLoss: Number(e.target.value)})}
                  className="input flex-1"
                />
                <span className="text-sm text-text-secondary">₽</span>
              </div>
            </div>

            <div>
              <label className="text-xs text-text-secondary mb-1 block">Мин. профит вилки</label>
              <div className="flex items-center gap-2">
                <input 
                  type="number" 
                  value={settings.minProfit}
                  onChange={e => setSettings({...settings, minProfit: Number(e.target.value)})}
                  className="input flex-1"
                />
                <span className="text-sm text-text-secondary">%</span>
              </div>
            </div>
          </div>

          {/* Restrictions */}
          <div className="space-y-4">
            <h4 className="text-sm font-medium text-text-secondary flex items-center gap-2">
              <Timer size={14} /> Ограничения
            </h4>

            <div>
              <label className="text-xs text-text-secondary mb-1 block">Макс. ставок в день</label>
              <input 
                type="number" 
                value={settings.maxBetsPerDay}
                onChange={e => setSettings({...settings, maxBetsPerDay: Number(e.target.value)})}
                className="input w-full"
              />
            </div>

            <div>
              <label className="text-xs text-text-secondary mb-1 block">Пауза после ставки (сек)</label>
              <input 
                type="number" 
                value={settings.cooldownAfterBet}
                onChange={e => setSettings({...settings, cooldownAfterBet: Number(e.target.value)})}
                className="input w-full"
              />
            </div>

            <div>
              <label className="text-xs text-text-secondary mb-1 block">Пауза после проигрыша (мин)</label>
              <input 
                type="number" 
                value={settings.cooldownAfterLoss}
                onChange={e => setSettings({...settings, cooldownAfterLoss: Number(e.target.value)})}
                className="input w-full"
              />
            </div>

            <div>
              <label className="text-xs text-text-secondary mb-1 block">Макс. одновременных ставок</label>
              <input 
                type="number" 
                value={settings.maxConcurrentBets}
                onChange={e => setSettings({...settings, maxConcurrentBets: Number(e.target.value)})}
                className="input w-full"
              />
            </div>
          </div>
        </div>

        {/* Security */}
        <div className="mt-6 pt-5 border-t border-border">
          <h4 className="text-sm font-medium text-text-secondary flex items-center gap-2 mb-4">
            <Shield size={14} /> Безопасность
          </h4>
          
          <div className="space-y-3">
            <label className="flex items-center justify-between cursor-pointer">
              <span className="text-sm text-text-primary">Требовать подтверждение</span>
              <input 
                type="checkbox" 
                checked={settings.requireConfirmation}
                onChange={e => setSettings({...settings, requireConfirmation: e.target.checked})}
                className="w-4 h-4 rounded accent-accent"
              />
            </label>
            
            {settings.requireConfirmation && (
              <div className="ml-6">
                <label className="text-xs text-text-secondary mb-1 block">Таймаут подтверждения (сек)</label>
                <input 
                  type="number" 
                  value={settings.confirmationTimeout}
                  onChange={e => setSettings({...settings, confirmationTimeout: Number(e.target.value)})}
                  className="input w-32"
                />
              </div>
            )}

            <label className="flex items-center justify-between cursor-pointer">
              <span className="text-sm text-text-primary">Остановить при серии проигрышей</span>
              <input 
                type="checkbox" 
                checked={settings.stopAfterLosses}
                onChange={e => setSettings({...settings, stopAfterLosses: e.target.checked})}
                className="w-4 h-4 rounded accent-accent"
              />
            </label>

            {settings.stopAfterLosses && (
              <div className="ml-6">
                <label className="text-xs text-text-secondary mb-1 block">После скольких проигрышей</label>
                <input 
                  type="number" 
                  value={settings.stopAfterLossesCount}
                  onChange={e => setSettings({...settings, stopAfterLossesCount: Number(e.target.value)})}
                  className="input w-32"
                />
              </div>
            )}
          </div>
        </div>

        {/* Notifications */}
        <div className="mt-6 pt-5 border-t border-border">
          <h4 className="text-sm font-medium text-text-secondary flex items-center gap-2 mb-4">
            <Bell size={14} /> Уведомления
          </h4>
          
          <div className="grid grid-cols-2 gap-3">
            {[
              { key: 'notifyOnBet', label: 'О ставке' },
              { key: 'notifyOnWin', label: 'О выигрыше' },
              { key: 'notifyOnLoss', label: 'О проигрыше' },
              { key: 'notifyOnError', label: 'Об ошибке' }
            ].map(({ key, label }) => (
              <label key={key} className="flex items-center gap-2 cursor-pointer">
                <input 
                  type="checkbox" 
                  checked={settings[key as keyof typeof settings] as boolean}
                  onChange={e => setSettings({...settings, [key]: e.target.checked})}
                  className="w-4 h-4 rounded accent-accent"
                />
                <span className="text-sm text-text-primary">{label}</span>
              </label>
            ))}
          </div>
        </div>
      </motion.div>

      {/* Pending Queue (for semi-auto) */}
      {mode === 'semi_auto' && (
        <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
          <h3 className="text-base font-semibold text-text-primary mb-4 flex items-center gap-2">
            <Clock size={18} className="text-accent" /> Очередь на подтверждение
          </h3>
          
          {pendingBets.length > 0 ? (
            <div className="space-y-3">
              {pendingBets.map(bet => (
                <div key={bet.id} className="flex items-center gap-4 p-4 rounded-lg bg-background">
                  <div className="flex-1">
                    <div className="text-sm font-medium text-text-primary">{bet.match}</div>
                    <div className="text-xs text-text-secondary">Профит: +{bet.profit}% • Ставка: {bet.stake.toLocaleString()} ₽</div>
                  </div>
                  <div className="flex gap-2">
                    <button className="btn btn-primary text-xs px-3 py-1.5">Подтвердить</button>
                    <button className="btn btn-secondary text-xs px-3 py-1.5">Пропустить</button>
                  </div>
                  <div className="text-xs text-red-400 flex items-center gap-1">
                    <Timer size={12} /> {bet.timeLeft}с
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-center py-8 text-text-secondary">
              <Clock size={32} className="mx-auto opacity-20 mb-2" />
              <p>Очередь пуста</p>
            </div>
          )}
        </motion.div>
      )}

      {/* Stats */}
      <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
        <h3 className="text-base font-semibold text-text-primary mb-4">Статистика</h3>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          {[
            { label: 'Ставок сегодня', value: demoStats.betsToday },
            { label: 'Выигрышей', value: demoStats.wins, color: 'text-emerald-400' },
            { label: 'Проигрышей', value: demoStats.losses, color: 'text-red-400' },
            { label: 'Профит', value: `+${demoStats.profit.toLocaleString()} ₽`, color: 'text-emerald-400' },
            { label: 'Win Rate', value: `${demoStats.winRate}%` },
            { label: 'Средний профит', value: `${demoStats.avgProfit}%` },
            { label: 'Лучшая вилка', value: `${demoStats.bestFork}%` },
            { label: 'Ошибок', value: demoStats.errors, color: 'text-amber-400' }
          ].map((stat, i) => (
            <div key={i} className="text-center p-3 rounded-lg bg-background">
              <div className={`text-xl font-bold ${stat.color || 'text-text-primary'}`}>{stat.value}</div>
              <div className="text-xs text-text-secondary mt-1">{stat.label}</div>
            </div>
          ))}
        </div>
      </motion.div>

      {/* Action Log */}
      <motion.div variants={item} className="rounded-card border border-border bg-surface p-5">
        <h3 className="text-base font-semibold text-text-primary mb-4">Лог действий</h3>
        <div className="space-y-2">
          {actionLogs.map(log => (
            <div key={log.id} className="flex items-center gap-3 py-2 px-3 rounded-lg hover:bg-background transition-colors">
              {log.type === 'success' && <Check size={14} className="text-emerald-400 shrink-0" />}
              {log.type === 'win' && <TrendingUp size={14} className="text-emerald-400 shrink-0" />}
              {log.type === 'error' && <AlertTriangle size={14} className="text-red-400 shrink-0" />}
              {log.type === 'info' && <Zap size={14} className="text-blue-400 shrink-0" />}
              <div className="flex-1 min-w-0">
                <div className="text-sm text-text-primary">{log.action}</div>
                <div className="text-xs text-text-secondary">{log.detail}</div>
              </div>
              <div className="text-xs text-text-muted shrink-0">{log.time}</div>
            </div>
          ))}
        </div>
      </motion.div>
    </motion.div>
  )
}
