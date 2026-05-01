import { useState, useEffect, useCallback } from 'react';
import { 
  Play, 
  Pause, 
  Settings, 
  AlertTriangle, 
  CheckCircle2, 
  XCircle,
  Clock,
  Zap,
  Hand,
  MousePointer,
  ChevronDown,
  RefreshCw,
  Shield,
  TrendingUp,
  Wallet
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

// Types
export type ExecutionMode = 'auto' | 'semi' | 'manual';

export interface ExecutionState {
  mode: ExecutionMode;
  isRunning: boolean;
  isPaused: boolean;
  activeForks: number;
  pendingConfirmations: number;
  totalBetsToday: number;
  profitToday: number;
  bankroll: number;
  maxStake: number;
  currentStake: number;
  lastError?: string;
}

export interface PendingBet {
  id: string;
  forkId: string;
  bookmaker: string;
  event: string;
  market: string;
  selection: string;
  odds: number;
  stake: number;
  profit: number;
  screenshot?: string;
  expiresAt: number;
}

export function ExecutionPanel() {
  const [state, setState] = useState<ExecutionState>({
    mode: 'semi',
    isRunning: false,
    isPaused: false,
    activeForks: 0,
    pendingConfirmations: 0,
    totalBetsToday: 0,
    profitToday: 0,
    bankroll: 100000,
    maxStake: 5000,
    currentStake: 1000,
  });

  const [pendingBets, setPendingBets] = useState<PendingBet[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [selectedBet, setSelectedBet] = useState<PendingBet | null>(null);
  const [logs, setLogs] = useState<string[]>([]);

  // WebSocket connection for real-time updates
  useEffect(() => {
    const ws = new WebSocket('ws://localhost:8080/ws/execution');
    
    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      handleWebSocketMessage(data);
    };

    ws.onclose = () => {
      addLog('WebSocket disconnected, retrying...');
    };

    return () => ws.close();
  }, []);

  const handleWebSocketMessage = (data: any) => {
    switch (data.type) {
      case 'state_update':
        setState(prev => ({ ...prev, ...data.payload }));
        break;
      case 'pending_bet':
        setPendingBets(prev => [...prev, data.payload]);
        addLog(`New pending bet: ${data.payload.bookmaker} - ${data.payload.event}`);
        break;
      case 'bet_confirmed':
        setPendingBets(prev => prev.filter(b => b.id !== data.payload.id));
        addLog(`Bet confirmed: ${data.payload.bookmaker}`);
        break;
      case 'bet_rejected':
        setPendingBets(prev => prev.filter(b => b.id !== data.payload.id));
        addLog(`Bet rejected: ${data.payload.bookmaker} - ${data.payload.reason}`);
        break;
      case 'error':
        addLog(`Error: ${data.payload.message}`);
        break;
    }
  };

  const addLog = (message: string) => {
    const timestamp = new Date().toLocaleTimeString('ru-RU');
    setLogs(prev => [`[${timestamp}] ${message}`, ...prev].slice(0, 100));
  };

  const handleStart = async () => {
    try {
      const response = await fetch('/api/v1/autobet/start', { method: 'POST' });
      if (response.ok) {
        setState(prev => ({ ...prev, isRunning: true, isPaused: false }));
        addLog('Execution started in ' + state.mode + ' mode');
      }
    } catch (error) {
      addLog('Failed to start execution');
    }
  };

  const handleStop = async () => {
    try {
      const response = await fetch('/api/v1/autobet/stop', { method: 'POST' });
      if (response.ok) {
        setState(prev => ({ ...prev, isRunning: false }));
        addLog('Execution stopped');
      }
    } catch (error) {
      addLog('Failed to stop execution');
    }
  };

  const handlePause = () => {
    setState(prev => ({ ...prev, isPaused: !prev.isPaused }));
    addLog(state.isPaused ? 'Execution resumed' : 'Execution paused');
  };

  const handleModeChange = async (mode: ExecutionMode) => {
    setState(prev => ({ ...prev, mode }));
    addLog(`Mode changed to: ${mode}`);
    
    // Send to backend
    try {
      await fetch('/api/v1/execution/mode', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ mode }),
      });
    } catch (error) {
      console.error('Failed to update mode:', error);
    }
  };

  const handleConfirmBet = async (betId: string) => {
    try {
      const response = await fetch(`/api/v1/execution/confirm/${betId}`, {
        method: 'POST',
      });
      
      if (response.ok) {
        setPendingBets(prev => prev.filter(b => b.id !== betId));
        addLog(`Bet ${betId} confirmed`);
      }
    } catch (error) {
      addLog(`Failed to confirm bet ${betId}`);
    }
  };

  const handleRejectBet = async (betId: string) => {
    try {
      const response = await fetch(`/api/v1/execution/reject/${betId}`, {
        method: 'POST',
      });
      
      if (response.ok) {
        setPendingBets(prev => prev.filter(b => b.id !== betId));
        addLog(`Bet ${betId} rejected`);
      }
    } catch (error) {
      addLog(`Failed to reject bet ${betId}`);
    }
  };

  const getModeIcon = (mode: ExecutionMode) => {
    switch (mode) {
      case 'auto': return <Zap size={16} />;
      case 'semi': return <Hand size={16} />;
      case 'manual': return <MousePointer size={16} />;
    }
  };

  const getModeLabel = (mode: ExecutionMode) => {
    switch (mode) {
      case 'auto': return 'Авто';
      case 'semi': return 'Полуавто';
      case 'manual': return 'Ручной';
    }
  };

  const getModeDescription = (mode: ExecutionMode) => {
    switch (mode) {
      case 'auto': return 'Ставки без подтверждения';
      case 'semi': return 'Подтверждение каждой ставки';
      case 'manual': return 'Только подготовка купона';
    }
  };

  return (
    <div className="execution-panel">
      {/* Header with controls */}
      <div className="execution-header">
        <div className="mode-selector">
          <span className="label">Режим:</span>
          <div className="mode-buttons">
            {(['auto', 'semi', 'manual'] as ExecutionMode[]).map(mode => (
              <button
                key={mode}
                className={`mode-btn ${state.mode === mode ? 'active' : ''}`}
                onClick={() => handleModeChange(mode)}
                disabled={state.isRunning}
                title={getModeDescription(mode)}
              >
                {getModeIcon(mode)}
                {getModeLabel(mode)}
              </button>
            ))}
          </div>
        </div>

        <div className="main-controls">
          {!state.isRunning ? (
            <button className="btn-start" onClick={handleStart}>
              <Play size={18} />
              Старт
            </button>
          ) : (
            <>
              <button 
                className={`btn-pause ${state.isPaused ? 'active' : ''}`}
                onClick={handlePause}
              >
                {state.isPaused ? <Play size={18} /> : <Pause size={18} />}
                {state.isPaused ? 'Продолжить' : 'Пауза'}
              </button>
              <button className="btn-stop" onClick={handleStop}>
                <XCircle size={18} />
                Стоп
              </button>
            </>
          )}
          
          <button 
            className="btn-settings"
            onClick={() => setShowSettings(!showSettings)}
          >
            <Settings size={18} />
          </button>
        </div>
      </div>

      {/* Status bar */}
      <div className="status-bar">
        <div className={`status-indicator ${state.isRunning ? 'running' : 'stopped'}`}>
          <div className="status-dot" />
          <span>{state.isRunning ? (state.isPaused ? 'Пауза' : 'Работает') : 'Остановлено'}</span>
        </div>
        
        <div className="status-stats">
          <div className="stat">
            <Clock size={14} />
            <span>Активных вилок: {state.activeForks}</span>
          </div>
          <div className="stat">
            <AlertTriangle size={14} />
            <span>Ожидают: {state.pendingConfirmations}</span>
          </div>
          <div className="stat">
            <CheckCircle2 size={14} />
            <span>Ставок сегодня: {state.totalBetsToday}</span>
          </div>
          <div className="stat profit">
            <TrendingUp size={14} />
            <span>Профит: {state.profitToday.toFixed(2)} ₽</span>
          </div>
        </div>
      </div>

      {/* Main content */}
      <div className="execution-content">
        {/* Pending bets queue */}
        <div className="pending-bets-section">
          <div className="section-header">
            <h3>Очередь подтверждения</h3>
            <span className="badge">{pendingBets.length}</span>
          </div>
          
          {pendingBets.length === 0 ? (
            <div className="empty-queue">
              <Shield size={48} />
              <p>Нет ожидающих ставок</p>
              <span>В режиме {getModeLabel(state.mode)} ставки будут {state.mode === 'auto' ? 'приниматься автоматически' : 'требовать подтверждения'}</span>
            </div>
          ) : (
            <div className="pending-bets-list">
              {pendingBets.map(bet => (
                <PendingBetCard
                  key={bet.id}
                  bet={bet}
                  mode={state.mode}
                  onConfirm={() => handleConfirmBet(bet.id)}
                  onReject={() => handleRejectBet(bet.id)}
                  onClick={() => setSelectedBet(bet)}
                />
              ))}
            </div>
          )}
        </div>

        {/* Logs */}
        <div className="logs-section">
          <div className="section-header">
            <h3>Лог выполнения</h3>
            <button className="btn-clear" onClick={() => setLogs([])}>
              Очистить
            </button>
          </div>
          <div className="logs-container">
            {logs.map((log, i) => (
              <div key={i} className="log-line">{log}</div>
            ))}
          </div>
        </div>
      </div>

      {/* Settings panel */}
      <AnimatePresence>
        {showSettings && (
          <SettingsPanel
            state={state}
            onUpdate={setState}
            onClose={() => setShowSettings(false)}
          />
        )}
      </AnimatePresence>

      {/* Bet detail modal */}
      <AnimatePresence>
        {selectedBet && (
          <BetDetailModal
            bet={selectedBet}
            onClose={() => setSelectedBet(null)}
            onConfirm={() => handleConfirmBet(selectedBet.id)}
            onReject={() => handleRejectBet(selectedBet.id)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

// Sub-components
function PendingBetCard({ 
  bet, 
  mode, 
  onConfirm, 
  onReject,
  onClick 
}: { 
  bet: PendingBet; 
  mode: ExecutionMode;
  onConfirm: () => void;
  onReject: () => void;
  onClick: () => void;
}) {
  const timeLeft = Math.max(0, Math.floor((bet.expiresAt - Date.now()) / 1000));
  const isExpiring = timeLeft < 10;

  return (
    <motion.div 
      className="pending-bet-card"
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, x: -100 }}
      onClick={onClick}
    >
      <div className="bet-header">
        <div className="bet-bookmaker">{bet.bookmaker}</div>
        <div className={`bet-timer ${isExpiring ? 'expiring' : ''}`}>
          <Clock size={12} />
          {timeLeft}s
        </div>
      </div>
      
      <div className="bet-event">{bet.event}</div>
      <div className="bet-market">{bet.market} — {bet.selection}</div>
      
      <div className="bet-details">
        <div className="bet-odds">Кэф: {bet.odds.toFixed(2)}</div>
        <div className="bet-stake">{bet.stake.toFixed(0)} ₽</div>
        <div className="bet-profit">+{bet.profit.toFixed(2)} ₽</div>
      </div>

      {mode === 'semi' && (
        <div className="bet-actions" onClick={e => e.stopPropagation()}>
          <button className="btn-confirm" onClick={onConfirm}>
            <CheckCircle2 size={14} />
            Подтвердить
          </button>
          <button className="btn-reject" onClick={onReject}>
            <XCircle size={14} />
            Отклонить
          </button>
        </div>
      )}
    </motion.div>
  );
}

function SettingsPanel({ 
  state, 
  onUpdate, 
  onClose 
}: { 
  state: ExecutionState; 
  onUpdate: (s: ExecutionState) => void;
  onClose: () => void;
}) {
  const [localState, setLocalState] = useState(state);

  const handleSave = () => {
    onUpdate(localState);
    onClose();
  };

  return (
    <motion.div 
      className="settings-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={onClose}
    >
      <motion.div 
        className="settings-panel"
        initial={{ scale: 0.9, y: 20 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.9, y: 20 }}
        onClick={e => e.stopPropagation()}
      >
        <div className="settings-header">
          <h3>Настройки выполнения</h3>
          <button className="btn-close" onClick={onClose}>×</button>
        </div>

        <div className="settings-content">
          <div className="setting-group">
            <label>Банкролл (₽)</label>
            <input 
              type="number"
              value={localState.bankroll}
              onChange={e => setLocalState(prev => ({ ...prev, bankroll: Number(e.target.value) }))}
            />
          </div>

          <div className="setting-group">
            <label>Максимальная ставка (₽)</label>
            <input 
              type="number"
              value={localState.maxStake}
              onChange={e => setLocalState(prev => ({ ...prev, maxStake: Number(e.target.value) }))}
            />
          </div>

          <div className="setting-group">
            <label>Текущая ставка (₽)</label>
            <input 
              type="number"
              value={localState.currentStake}
              onChange={e => setLocalState(prev => ({ ...prev, currentStake: Number(e.target.value) }))}
            />
          </div>

          <div className="setting-group checkbox">
            <label>
              <input type="checkbox" defaultChecked />
              Остановить при ошибке
            </label>
          </div>

          <div className="setting-group checkbox">
            <label>
              <input type="checkbox" defaultChecked />
              Отправлять уведомления в Telegram
            </label>
          </div>
        </div>

        <div className="settings-actions">
          <button className="btn-secondary" onClick={onClose}>Отмена</button>
          <button className="btn-primary" onClick={handleSave}>Сохранить</button>
        </div>
      </motion.div>
    </motion.div>
  );
}

function BetDetailModal({ 
  bet, 
  onClose, 
  onConfirm, 
  onReject 
}: { 
  bet: PendingBet;
  onClose: () => void;
  onConfirm: () => void;
  onReject: () => void;
}) {
  return (
    <motion.div 
      className="modal-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={onClose}
    >
      <motion.div 
        className="modal-content bet-detail"
        initial={{ scale: 0.9, y: 20 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.9, y: 20 }}
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>Подтверждение ставки</h3>
          <button className="btn-close" onClick={onClose}>×</button>
        </div>

        <div className="bet-detail-content">
          <div className="detail-row">
            <span className="label">Букмекер:</span>
            <span className="value">{bet.bookmaker}</span>
          </div>
          <div className="detail-row">
            <span className="label">Событие:</span>
            <span className="value">{bet.event}</span>
          </div>
          <div className="detail-row">
            <span className="label">Рынок:</span>
            <span className="value">{bet.market}</span>
          </div>
          <div className="detail-row">
            <span className="label">Выбор:</span>
            <span className="value">{bet.selection}</span>
          </div>
          <div className="detail-row">
            <span className="label">Коэффициент:</span>
            <span className="value">{bet.odds.toFixed(2)}</span>
          </div>
          <div className="detail-row">
            <span className="label">Сумма:</span>
            <span className="value">{bet.stake.toFixed(0)} ₽</span>
          </div>
          <div className="detail-row profit">
            <span className="label">Профит:</span>
            <span className="value">+{bet.profit.toFixed(2)} ₽</span>
          </div>

          {bet.screenshot && (
            <div className="screenshot-container">
              <img src={`data:image/png;base64,${bet.screenshot}`} alt="Coupon screenshot" />
            </div>
          )}
        </div>

        <div className="modal-actions">
          <button className="btn-secondary" onClick={onReject}>
            <XCircle size={16} />
            Отклонить
          </button>
          <button className="btn-primary" onClick={onConfirm}>
            <CheckCircle2 size={16} />
            Подтвердить ставку
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

export default ExecutionPanel;
