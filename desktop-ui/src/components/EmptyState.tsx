import { motion } from 'framer-motion'
import { type LucideIcon } from 'lucide-react'

interface EmptyStateProps {
  icon: LucideIcon
  title: string
  subtitle: string
  action?: {
    label: string
    onClick: () => void
  }
}

export function EmptyState({ icon: Icon, title, subtitle, action }: EmptyStateProps) {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.3 }}
      className="flex flex-col items-center justify-center py-16 px-4"
    >
      <div className="relative mb-6">
        <div className="absolute inset-0 bg-accent/20 blur-xl rounded-full" />
        <div className="relative p-4 bg-surface rounded-2xl border border-border">
          <Icon size={48} className="text-text-muted" strokeWidth={1} />
        </div>
      </div>
      
      <h3 className="text-lg font-semibold text-text-primary mb-2">{title}</h3>
      <p className="text-sm text-text-secondary mb-6 text-center max-w-sm">{subtitle}</p>
      
      {action && (
        <button
          onClick={action.onClick}
          className="btn btn-primary text-sm px-4 py-2 rounded-button"
        >
          {action.label}
        </button>
      )}
    </motion.div>
  )
}
