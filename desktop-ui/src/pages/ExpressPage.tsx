import { motion } from 'framer-motion'
import { Layers, Target, Zap, AlertTriangle, CheckCircle, Info } from 'lucide-react'
import { toast } from 'sonner'
import type { ExpressFork } from '../types'

interface ExpressPageProps {
  expressForks: ExpressFork[]
}

const riskConfig = {
  low: { label: 'Низкий', color: 'var(--accent-green)', icon: CheckCircle },
  medium: { label: 'Средний', color: 'var(--accent-yellow)', icon: Info },
  high: { label: 'Высокий', color: 'var(--accent-red)', icon: AlertTriangle },
}

export function ExpressPage({ expressForks }: ExpressPageProps) {
  return (
    <motion.div 
      className="space-y-6"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Экспресс-вилки</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Комбинирование нескольких событий для повышения прибыли
          </p>
        </div>
      </div>

      {expressForks.length > 0 ? (
        <div className="space-y-4">
          {expressForks.map((exp, i) => {
            const RiskIcon = riskConfig[exp.risk].icon
            return (
              <motion.div 
                key={exp.id} 
                className="glass-card p-5"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: i * 0.1 }}
              >
                <div className="flex items-start justify-between mb-4">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'var(--gradient-primary)' }}>
                      <Layers size={20} color="#fff" />
                    </div>
                    <div>
                      <h3 className="text-base font-semibold">Экспресс #{i + 1}</h3>
                      <div className="flex items-center gap-2 mt-1">
                        <RiskIcon size={14} style={{ color: riskConfig[exp.risk].color }} />
                        <span className="text-xs" style={{ color: riskConfig[exp.risk].color }}>
                          Риск: {riskConfig[exp.risk].label}
                        </span>
                      </div>
                    </div>
                  </div>
                  <div className="text-right">
                    <div className="flex items-center gap-1.5">
                      <Zap size={16} style={{ color: 'var(--accent-green)' }} />
                      <span className="profit profit-positive text-lg">+{exp.profit_percent.toFixed(2)}%</span>
                    </div>
                    <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                      Объём: {exp.total_stake.toLocaleString()}₽
                    </p>
                  </div>
                </div>

                <div className="space-y-2 mb-4">
                  {exp.legs.map((leg, j) => (
                    <div key={j} className="flex items-center justify-between p-3 rounded-lg" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                      <div className="flex items-center gap-3">
                        <div className="w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold" style={{ background: 'var(--gradient-primary)', color: '#fff' }}>
                          {j + 1}
                        </div>
                        <div>
                          <p className="text-sm font-medium">{leg.event}</p>
                          <p className="text-xs mt-0.5" style={{ color: 'var(--text-secondary)' }}>
                            {leg.market} → {leg.selection}
                          </p>
                        </div>
                      </div>
                      <div className="text-right">
                        <p className="text-sm font-medium capitalize">{leg.bookmaker}</p>
                        <p className="text-sm font-mono">@ {leg.odds.toFixed(2)}</p>
                      </div>
                    </div>
                  ))}
                </div>

                <div className="flex items-center justify-between p-3 rounded-lg mb-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                  <div className="flex items-center gap-2">
                    <Target size={16} style={{ color: 'var(--accent-blue)' }} />
                    <span className="text-sm">Общий коэффициент</span>
                  </div>
                  <span className="text-lg font-mono font-bold" style={{ color: 'var(--accent-blue)' }}>
                    {exp.legs.reduce((acc, leg) => acc * leg.odds, 1).toFixed(2)}
                  </span>
                </div>

                <button 
                  onClick={() => toast.success('Экспресс скопирован')}
                  className="btn btn-primary w-full justify-center"
                >
                  <Layers size={16} />
                  Разместить экспресс
                </button>
              </motion.div>
            )
          })}
        </div>
      ) : (
        <div className="glass-card p-16 text-center">
          <motion.div
            animate={{ scale: [1, 1.1, 1] }}
            transition={{ duration: 2, repeat: Infinity }}
          >
            <Layers size={64} className="mx-auto mb-4 opacity-30" style={{ color: 'var(--accent-cyan)' }} />
          </motion.div>
          <h3 className="text-xl font-bold mb-2">Экспресс-вилки в разработке</h3>
          <p className="text-sm mb-2" style={{ color: 'var(--text-muted)' }}>
            Автоматический поиск комбинаций из 2-3 событий
          </p>
          <p className="text-xs mb-6" style={{ color: 'var(--text-muted)' }}>
            Подробнее: <a href="https://bkvilki.ru/articles/vilki_na_ekspressah" target="_blank" rel="noopener noreferrer" className="underline" style={{ color: 'var(--accent-blue)' }}>bkvilki.ru</a>
          </p>
        </div>
      )}
    </motion.div>
  )
}
