import { useState, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Sidebar } from './components/Sidebar'
import { Dashboard } from './pages/Dashboard'
import { SurebetsPage } from './pages/SurebetsPage'
import { CorridorsPage } from './pages/CorridorsPage'
import { ExpressPage } from './pages/ExpressPage'
import { HistoryPage } from './pages/HistoryPage'
import { SettingsPage } from './pages/SettingsPage'
import { useScanner } from './hooks/useScanner'
import type { TabType } from './types'

function App() {
  const [activeTab, setActiveTab] = useState<TabType>('dashboard')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const { connected, surebets, metrics, bookmakers, corridors, expressForks } = useScanner()

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        switch(e.key) {
          case '1': e.preventDefault(); setActiveTab('dashboard'); break
          case '2': e.preventDefault(); setActiveTab('surebets'); break
          case '3': e.preventDefault(); setActiveTab('corridors'); break
          case '4': e.preventDefault(); setActiveTab('express'); break
          case '5': e.preventDefault(); setActiveTab('history'); break
          case '6': e.preventDefault(); setActiveTab('settings'); break
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const renderPage = () => {
    switch(activeTab) {
      case 'dashboard':
        return <Dashboard metrics={metrics} surebets={surebets} bookmakers={bookmakers} />
      case 'surebets':
        return <SurebetsPage surebets={surebets} />
      case 'corridors':
        return <CorridorsPage corridors={corridors} />
      case 'express':
        return <ExpressPage expressForks={expressForks} />
      case 'history':
        return <HistoryPage />
      case 'settings':
        return <SettingsPage />
      default:
        return <Dashboard metrics={metrics} surebets={surebets} bookmakers={bookmakers} />
    }
  }

  return (
    <div className="flex h-screen overflow-hidden" style={{ background: 'var(--bg-primary)' }}>
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        collapsed={sidebarCollapsed}
        onToggle={() => setSidebarCollapsed(!sidebarCollapsed)}
        wsConnected={connected}
        scannerRunning={metrics !== null}
      />

      <main className="flex-1 overflow-auto">
        <AnimatePresence mode="wait">
          <motion.div
            key={activeTab}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.2 }}
            className="h-full overflow-auto"
          >
            <div className="p-6 max-w-[1920px] mx-auto">
              {renderPage()}
            </div>
          </motion.div>
        </AnimatePresence>
      </main>
    </div>
  )
}

export default App
