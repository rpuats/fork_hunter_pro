import { useState, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Sidebar } from './components/Sidebar'
import { Dashboard } from './pages/Dashboard'
import { SurebetsPage } from './pages/SurebetsPage'
import { CorridorsPage } from './pages/CorridorsPage'
import { ExpressPage } from './pages/ExpressPage'
import { OperatorPage } from './pages/OperatorPage'
import { AccountsPage } from './pages/AccountsPage'
import { HistoryPage } from './pages/HistoryPage'
import { SettingsPage } from './pages/SettingsPage'
import { useScanner } from './hooks/useScanner'
import type { TabType } from './types'

function App() {
  const [activeTab, setActiveTab] = useState<TabType>('dashboard')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [accountFocus, setAccountFocus] = useState<string | null>(null)
  const { connected, scannerStatus, surebets, metrics, bookmakers, corridors, expressForks, valueBets, generosityIndices, executionOverview, executionLedger, executionState, executionOperatorQueue, semiAutoCoupons, confirmSemiAutoCoupon, bootstrapAccountSession, refreshAccountBalance, updateAccountControl, parserCoverage, parserHealth, accounts, accountsSummary, bankrollState, bankrollRecommendations, freebetSummary, bookmakerStatusCatalog } = useScanner()

  const openAccountsFocus = (bookmaker: string) => {
    setAccountFocus(bookmaker)
    setActiveTab('accounts')
  }

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        switch(e.key) {
          case '1': e.preventDefault(); setActiveTab('dashboard'); break
          case '2': e.preventDefault(); setActiveTab('surebets'); break
          case '3': e.preventDefault(); setActiveTab('corridors'); break
          case '4': e.preventDefault(); setActiveTab('express'); break
          case '5': e.preventDefault(); setActiveTab('operator'); break
          case '6': e.preventDefault(); setActiveTab('accounts'); break
          case '7': e.preventDefault(); setActiveTab('history'); break
          case '8': e.preventDefault(); setActiveTab('settings'); break
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const renderPage = () => {
    switch(activeTab) {
      case 'dashboard':
        return <Dashboard />
      case 'surebets':
        return <SurebetsPage />
      case 'corridors':
        return <CorridorsPage />
      case 'express':
        return <ExpressPage />
      case 'operator':
        return <OperatorPage />
      case 'accounts':
        return <AccountsPage />
      case 'history':
        return <HistoryPage />
      case 'settings':
        return <SettingsPage />
      default:
        return <Dashboard />
    }
  }

  return (
    <div className="flex h-screen overflow-hidden bg-background text-text-primary font-sans">
      <Sidebar
        activeTab={activeTab}
        onTabChange={setActiveTab}
        collapsed={sidebarCollapsed}
        onToggle={() => setSidebarCollapsed(!sidebarCollapsed)}
        wsConnected={connected}
        scannerRunning={scannerStatus?.running ?? metrics !== null}
      />

      <main className="flex-1 overflow-auto">
        <AnimatePresence mode="wait">
          <motion.div
            key={activeTab}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
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
