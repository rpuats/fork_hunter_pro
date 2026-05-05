import { motion } from 'framer-motion'
import { 
  LayoutDashboard, Zap, GitBranch, Layers, History, Settings, Radar, Landmark,
  ChevronLeft, ChevronRight
} from 'lucide-react'
import type { TabType } from '../types'

interface SidebarProps {
  activeTab: TabType
  onTabChange: (tab: TabType) => void
  collapsed: boolean
  onToggle: () => void
  wsConnected: boolean
  scannerRunning: boolean
}

const tabs: { id: TabType; label: string; shortcut: string; icon: any; badge?: boolean }[] = [
  { id: 'dashboard', label: 'Обзор', shortcut: '⌘1', icon: LayoutDashboard },
  { id: 'surebets', label: 'Вилки', shortcut: '⌘2', icon: Zap, badge: true },
  { id: 'corridors', label: 'Коридоры', shortcut: '⌘3', icon: GitBranch },
  { id: 'express', label: 'Экспрессы', shortcut: '⌘4', icon: Layers },
  { id: 'operator', label: 'Авто-ставки', shortcut: '⌘5', icon: Radar },
  { id: 'accounts', label: 'Аккаунты', shortcut: '⌘6', icon: Landmark },
  { id: 'history', label: 'История', shortcut: '⌘7', icon: History },
  { id: 'settings', label: 'Настройки', shortcut: '⌘8', icon: Settings },
]

export function Sidebar({ activeTab, onTabChange, collapsed, onToggle, wsConnected, scannerRunning }: SidebarProps) {
  return (
    <motion.aside 
      className="relative flex flex-col border-r border-border bg-surface"
      animate={{ width: collapsed ? 64 : 240 }}
      transition={{ duration: 0.2, ease: 'easeOut' }}
    >
      {/* Toggle Button */}
      <button
        onClick={onToggle}
        className="absolute -right-3 top-6 z-50 w-6 h-6 rounded-full flex items-center justify-center bg-elevated border border-border text-text-secondary hover:text-text-primary transition-all duration-150 hover:scale-110"
      >
        {collapsed ? <ChevronRight size={14} /> : <ChevronLeft size={14} />}
      </button>

      {/* Logo */}
      <div className="p-4 border-b border-border">
        <div className="flex items-center gap-3">
          <motion.div 
            className="w-10 h-10 rounded-button flex items-center justify-center flex-shrink-0 bg-gradient-to-br from-accent to-accent-purple"
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            <Zap size={20} className="text-white" />
          </motion.div>
          
          {!collapsed && (
            <motion.div
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{ delay: 0.1 }}
            >
              <h1 className="text-base font-bold gradient-text">Ghost Imperium</h1>
              <p className="text-[10px] font-medium text-text-muted">v2.0 Pro</p>
            </motion.div>
          )}
        </div>
      </div>

      {/* Status Indicators */}
      {!collapsed && (
        <div className="px-4 py-3 border-b border-border space-y-2">
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${wsConnected ? 'animate-pulse' : ''}`} 
                 style={{ background: wsConnected ? '#10B981' : '#EF4444' }} />
            <span className="text-xs text-text-secondary">WebSocket</span>
          </div>
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${scannerRunning ? 'animate-pulse' : ''}`} 
                 style={{ background: scannerRunning ? '#10B981' : '#64748B' }} />
            <span className="text-xs text-text-secondary">Сканер</span>
          </div>
        </div>
      )}

      {/* Navigation */}
      <nav className="flex-1 p-2 space-y-0.5 overflow-y-auto">
        {tabs.map((tab, i) => {
          const Icon = tab.icon
          const isActive = activeTab === tab.id
          
          return (
            <motion.button
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              className={`w-full flex items-center gap-3 rounded-button transition-all duration-150 relative ${
                collapsed ? 'justify-center px-3 py-3' : 'px-3 py-2.5'
              } ${isActive ? 'bg-white/10 text-accent' : 'text-text-secondary hover:bg-white/5 hover:text-text-primary'}`}
              whileTap={{ scale: 0.98 }}
            >
              {isActive && !collapsed && (
                <motion.div
                  layoutId="activeIndicator"
                  className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-6 rounded-full bg-gradient-to-b from-accent to-accent-purple"
                  transition={{ type: 'spring', stiffness: 300, damping: 30 }}
                />
              )}
              
              <div className="relative">
                <Icon size={20} strokeWidth={1.5} />
                {tab.badge && collapsed && (
                  <span className="absolute -top-1 -right-1 w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                )}
              </div>
              
              {!collapsed && (
                <>
                  <span className="flex-1 text-left text-sm font-medium">{tab.label}</span>
                  
                  {tab.badge && (
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
                  )}
                  
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-background text-text-muted">
                    {tab.shortcut}
                  </span>
                </>
              )}
            </motion.button>
          )
        })}
      </nav>

      {/* Footer */}
      {!collapsed && (
        <div className="p-4 border-t border-border">
          <div className="text-[10px] text-center text-text-muted">
            © 2026 Ghost Imperium Pro
          </div>
        </div>
      )}
    </motion.aside>
  )
}
