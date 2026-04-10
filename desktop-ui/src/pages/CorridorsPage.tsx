import { motion } from 'framer-motion'
import { GitBranch, Target, TrendingUp, ArrowRight } from 'lucide-react'
import type { CorridorOpportunity } from '../types'

interface CorridorsPageProps {
  corridors: CorridorOpportunity[]
}

export function CorridorsPage({ corridors }: CorridorsPageProps) {
  return (
    <motion.div 
      className="space-y-6"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div>
        <h2 className="text-2xl font-bold">Коридоры</h2>
        <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
          Коридоры тоталов и фор — выигрыш при попадании в диапазон
        </p>
      </div>

      {corridors.length > 0 ? (
        <div className="space-y-4">
          {corridors.map((cor, i) => (
            <motion.div 
              key={cor.id} 
              className="glass-card p-5"
              initial={{ opacity: 0, y: 20 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: i * 0.1 }}
            >
              <div className="flex items-start justify-between mb-4">
                <div>
                  <h3 className="text-base font-semibold">{cor.home_team} — {cor.away_team}</h3>
                  <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>{cor.league}</p>
                </div>
                <div className="text-right">
                  <div className="flex items-center gap-1.5">
                    <TrendingUp size={16} style={{ color: 'var(--accent-green)' }} />
                    <span className="profit profit-positive text-lg">ROI {cor.expected_roi.toFixed(1)}%</span>
                  </div>
                  <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>
                    Вероятность {cor.double_win_probability * 100}%
                  </p>
                </div>
              </div>

              <div className="flex items-center gap-4 mb-4 p-3 rounded-lg" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <div className="flex-1">
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Нижняя граница</p>
                  <p className="text-sm font-mono">{cor.line_low}</p>
                </div>
                <ArrowRight size={20} style={{ color: 'var(--accent-blue)' }} />
                <div className="flex-1">
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Верхняя граница</p>
                  <p className="text-sm font-mono">{cor.line_high}</p>
                </div>
                <div className="flex-1">
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Коридор</p>
                  <p className="text-sm font-mono" style={{ color: 'var(--accent-green)' }}>{cor.line_high - cor.line_low}</p>
                </div>
              </div>

              <div className="flex gap-3">
                {cor.legs.map((leg, i) => (
                  <div key={i} className="flex-1 p-3 rounded-lg" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                    <p className="text-sm font-medium capitalize">{leg.bookmaker}</p>
                    <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{leg.selection}</p>
                    <p className="text-sm font-mono mt-1">@ {leg.odds.toFixed(2)}</p>
                  </div>
                ))}
              </div>
            </motion.div>
          ))}
        </div>
      ) : (
        <div className="glass-card p-16 text-center">
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 20, repeat: Infinity, ease: 'linear' }}
          >
            <GitBranch size={64} className="mx-auto mb-4 opacity-30" style={{ color: 'var(--accent-purple)' }} />
          </motion.div>
          <h3 className="text-xl font-bold mb-2">Коридоры в разработке</h3>
          <p className="text-sm" style={{ color: 'var(--text-muted)' }}>
            Скоро будет доступен поиск коридоров тоталов и фор
          </p>
        </div>
      )}
    </motion.div>
  )
}
