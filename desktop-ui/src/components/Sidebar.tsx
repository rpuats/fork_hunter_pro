import { motion } from 'framer-motion'
import { 
  LayoutDashboard, Zap, GitBranch, Layers, History, Settings, Radar, Landmark,
  ChevronLeft, ChevronRight, Circle 
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

const tabs: { id: TabType; label: string; shortcut: string; icon: any; badge?: { text: string; color: string } }[] = [
  { id: 'dashboard', label: 'Обзор', shortcut: '⌘1', icon: LayoutDashboard },
  { id: 'surebets', label: 'Вилки', shortcut: '⌘2', icon: Zap, badge: { text: 'LIVE', color: 'success' } },
  { id: 'corridors', label: 'Коридоры', shortcut: '⌘3', icon: GitBranch },
  { id: 'express', label: 'Экспрессы', shortcut: '⌘4', icon: Layers },
  { id: 'operator', label: 'Execution', shortcut: '⌘5', icon: Radar },
  { id: 'accounts', label: 'Accounts', shortcut: '⌘6', icon: Landmark },
  { id: 'history', label: 'История', shortcut: '⌘7', icon: History },
  { id: 'settings', label: 'Настройки', shortcut: '⌘8', icon: Settings },
]

export function Sidebar({ activeTab, onTabChange, collapsed, onToggle, wsConnected, scannerRunning }: SidebarProps) {
  return (
    <motion.aside 
      className="relative flex flex-col border-r"
      style={{ 
        width: collapsed ? 72 : 280, 
        background: 'var(--bg-secondary)', 
        borderColor: 'var(--border-color)' 
      }}
      animate={{ width: collapsed ? 72 : 280 }}
      transition={{ duration: 0.2, ease: 'easeInOut' }}
    >
      {/* Toggle Button */}
      <button
        onClick={onToggle}
        className="absolute -right-3 top-6 z-50 w-6 h-6 rounded-full flex items-center justify-center transition-all duration-200 hover:scale-110"
        style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)', color: 'var(--text-secondary)' }}
      >
        {collapsed ? <ChevronRight size={14} /> : <ChevronLeft size={14} />}
      </button>

      {/* Logo */}
      <div className="p-5 border-b" style={{ borderColor: 'var(--border-color)' }}>
        <div className="flex items-center gap-3">
          <motion.div 
            className="w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0"
            style={{ background: 'linear-gradient(135deg, #58a6ff 0%, #bc8cff 100%)' }}
            whileHover={{ scale: 1.05 }}
            whileTap={{ scale: 0.95 }}
          >
            <Zap size={20} color="#fff" />
          </motion.div>
          
          {!collapsed && (
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
            >
              <h1 className="text-base font-bold gradient-text">Ghost Imperium</h1>
              <p className="text-[10px] font-medium" style={{ color: 'var(--text-muted)' }}>v2.0 Pro</p>
            </motion.div>
          )}
        </div>
      </div>

      {/* Status */}
      {!collapsed && (
        <div className="px-4 py-3 border-b space-y-2" style={{ borderColor: 'var(--border-color)' }}>
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${wsConnected ? 'glow-live' : ''}`} 
                 style={{ background: wsConnected ? 'var(--accent-green)' : 'var(--accent-red)' }} />
            <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>WebSocket</span>
          </div>
          <div className="flex items-center gap-2">
            <div className={`w-2 h-2 rounded-full ${scannerRunning ? 'glow-live' : ''}`} 
                 style={{ background: scannerRunning ? 'var(--accent-green)' : 'var(--text-muted)' }} />
            <span className="text-xs" style={{ color: 'var(--text-secondary)' }}>Сканер</span>
          </div>
        </div>
      )}

      {/* Navigation */}
      <nav className="flex-1 p-3 space-y-1 overflow-y-auto">
        {tabs.map(tab => {
          const Icon = tab.icon
          const isActive = activeTab === tab.id
          
          return (
            <motion.button
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              className={`w-full flex items-center gap-3 rounded-lg transition-all duration-200 ${
                collapsed ? 'justify-center px-2 py-3' : 'px-3 py-2.5'
              }`}
              style={{
                background: isActive ? 'rgba(88, 166, 255, 0.1)' : 'transparent',
                color: isActive ? 'var(--accent-blue)' : 'var(--text-secondary)',
                border: isActive ? '1px solid rgba(88, 166, 255, 0.2)' : '1px solid transparent',
              }}
              whileHover={{ 
                background: 'var(--bg-hover)',
                color: 'var(--text-primary)'
              }}
              whileTap={{ scale: 0.98 }}
            >
              <Icon size={20} style={{ color: isActive ? 'var(--accent-blue)' : 'currentColor', flexShrink: 0 }} />
              
              {!collapsed && (
                <>
                  <span className="flex-1 text-left text-sm font-medium">{tab.label}</span>
                  
                  {tab.badge && (
                    <span className="badge badge-success text-[10px] px-1.5 py-0.5">
                      {tab.badge.text}
                    </span>
                  )}
                  
                  <span className="text-[10px] px-1.5 py-0.5 rounded" style={{ background: 'var(--bg-primary)', color: 'var(--text-muted)' }}>
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
        <div className="p-4 border-t" style={{ borderColor: 'var(--border-color)' }}>
          <div className="text-[10px] text-center" style={{ color: 'var(--text-muted)' }}>
            © 2026 Ghost Imperium Pro
          </div>
        </div>
      )}
    </motion.aside>
  )
}
