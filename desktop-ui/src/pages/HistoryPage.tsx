import { motion } from 'framer-motion'
import { History, Construction } from 'lucide-react'

export function HistoryPage() {
  return (
    <motion.div 
      className="space-y-6"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div>
        <h2 className="text-2xl font-bold">История</h2>
        <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
          История ставок, аналитика и статистика
        </p>
      </div>

      <div className="glass-card p-16 text-center">
        <History size={64} className="mx-auto mb-4 opacity-30" style={{ color: 'var(--accent-yellow)' }} />
        <h3 className="text-xl font-bold mb-2">История в разработке</h3>
        <p className="text-sm mb-6" style={{ color: 'var(--text-muted)' }}>
          Скоро будет доступна полная история всех вилок и ставок
        </p>
        <div className="flex items-center justify-center gap-2 px-4 py-2 rounded-lg" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)', display: 'inline-flex' }}>
          <Construction size={16} style={{ color: 'var(--accent-yellow)' }} />
          <span className="text-sm">В процессе разработки</span>
        </div>
      </div>
    </motion.div>
  )
}
