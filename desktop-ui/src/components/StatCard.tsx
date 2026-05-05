import { motion } from 'framer-motion'
import { type LucideIcon } from 'lucide-react'

interface StatCardProps {
  icon: LucideIcon
  label: string
  value: string | number
  trend?: string
  trendUp?: boolean
  color?: 'indigo' | 'green' | 'yellow' | 'red' | 'blue' | 'purple' | 'orange'
  delay?: number
}

const colorMap = {
  indigo: 'from-indigo-500/20 to-purple-500/20 text-indigo-400',
  green: 'from-emerald-500/20 to-teal-500/20 text-emerald-400',
  yellow: 'from-amber-500/20 to-orange-500/20 text-amber-400',
  red: 'from-red-500/20 to-rose-500/20 text-red-400',
  blue: 'from-blue-500/20 to-cyan-500/20 text-blue-400',
  purple: 'from-purple-500/20 to-pink-500/20 text-purple-400',
  orange: 'from-orange-500/20 to-red-500/20 text-orange-400',
}

const iconBgMap = {
  indigo: 'bg-indigo-500/10',
  green: 'bg-emerald-500/10',
  yellow: 'bg-amber-500/10',
  red: 'bg-red-500/10',
  blue: 'bg-blue-500/10',
  purple: 'bg-purple-500/10',
  orange: 'bg-orange-500/10',
}

export function StatCard({ icon: Icon, label, value, trend, trendUp, color = 'indigo', delay = 0 }: StatCardProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, delay }}
      whileHover={{ y: -2, boxShadow: '0 8px 12px -1px rgba(0,0,0,0.4)' }}
      className="relative p-5 rounded-card border border-border bg-surface overflow-hidden group"
    >
      <div className={`absolute top-0 left-0 right-0 h-0.5 bg-gradient-to-r ${colorMap[color]}`} />
      
      <div className="flex items-start justify-between mb-4">
        <div className={`p-2.5 rounded-lg ${iconBgMap[color]}`}>
          <Icon size={20} className={colorMap[color].split(' ').pop()} />
        </div>
        {trend && (
          <span className={`text-xs font-medium ${trendUp ? 'text-emerald-400' : 'text-red-400'}`}>
            {trendUp ? '↑' : '↓'} {trend}
          </span>
        )}
      </div>
      
      <div className="space-y-1">
        <p className="text-xs uppercase tracking-wider text-text-muted font-medium">{label}</p>
        <p className="text-2xl font-bold font-mono text-text-primary">{value}</p>
      </div>
      
      <div className="absolute inset-0 bg-gradient-to-br from-white/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
    </motion.div>
  )
}
