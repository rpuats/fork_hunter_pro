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
        return <Dashboard metrics={metrics} surebets={surebets} bookmakers={bookmakers} valueBets={valueBets} generosityIndices={generosityIndices} executionOverview={executionOverview} parserCoverage={parserCoverage} parserHealth={parserHealth} />
      case 'surebets':
        return <SurebetsPage surebets={surebets} />
      case 'corridors':
        return <CorridorsPage corridors={corridors} />
      case 'express':
        return <ExpressPage expressForks={expressForks} />
      case 'operator':
        return <OperatorPage executionOverview={executionOverview} executionLedger={executionLedger} executionState={executionState} executionOperatorQueue={executionOperatorQueue} semiAutoCoupons={semiAutoCoupons} onConfirmSemiAutoCoupon={confirmSemiAutoCoupon} parserCoverage={parserCoverage} parserHealth={parserHealth} bookmakers={bookmakers} bookmakerStatusCatalog={bookmakerStatusCatalog} accountStates={accounts} freebetSummary={freebetSummary} onOpenAccount={openAccountsFocus} />
      case 'accounts':
        return <AccountsPage accounts={accounts} accountsSummary={accountsSummary} bankrollState={bankrollState} bankrollRecommendations={bankrollRecommendations} executionState={executionState} focusedBookmaker={accountFocus} onBootstrapAccountSession={bootstrapAccountSession} onRefreshAccountBalance={refreshAccountBalance} onUpdateAccountControl={updateAccountControl} />
      case 'history':
        return <HistoryPage surebets={surebets} corridors={corridors} expressForks={expressForks} valueBets={valueBets} executionLedger={executionLedger} />
      case 'settings':
        return <SettingsPage />
      default:
        return <Dashboard metrics={metrics} surebets={surebets} bookmakers={bookmakers} valueBets={valueBets} generosityIndices={generosityIndices} executionOverview={executionOverview} parserCoverage={parserCoverage} parserHealth={parserHealth} />
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
        scannerRunning={scannerStatus?.running ?? metrics !== null}
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
