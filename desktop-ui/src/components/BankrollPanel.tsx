import { useState, useEffect } from 'react';
import { 
  Wallet, 
  TrendingUp, 
  AlertTriangle, 
  PieChart,
  ArrowRightLeft,
  RefreshCw,
  Settings,
  Plus,
  Minus
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

interface BankrollState {
  total: number;
  allocated: number;
  available: number;
  perBookmaker: Record<string, number>;
  strategy: 'equal_profit' | 'max_volume' | 'fixed';
  dailyProfit: number;
  dailyTarget: number;
  maxExposure: number;
}

interface BookmakerAllocation {
  id: string;
  name: string;
  allocated: number;
  available: number;
  inUse: number;
  status: 'ready' | 'low' | 'empty';
}

export function BankrollPanel() {
  const [state, setState] = useState<BankrollState>({
    total: 100000,
    allocated: 25000,
    available: 75000,
    perBookmaker: {},
    strategy: 'equal_profit',
    dailyProfit: 12500,
    dailyTarget: 50000,
    maxExposure: 0.3,
  });

  const [allocations, setAllocations] = useState<BookmakerAllocation[]>([
    { id: 'pari', name: 'Пари', allocated: 15000, available: 15000, inUse: 0, status: 'ready' },
    { id: 'fonbet', name: 'Фонбет', allocated: 10000, available: 10000, inUse: 0, status: 'ready' },
    { id: 'marathon', name: 'Марафон', allocated: 0, available: 0, inUse: 0, status: 'empty' },
  ]);

  const [showSettings, setShowSettings] = useState(false);
  const [editingBookmaker, setEditingBookmaker] = useState<string | null>(null);

  useEffect(() => {
    fetchBankrollState();
  }, []);

  const fetchBankrollState = async () => {
    try {
      const response = await fetch('/api/v1/bankroll');
      if (response.ok) {
        const data = await response.json();
        setState(data);
      }
    } catch (error) {
      console.error('Failed to fetch bankroll state:', error);
    }
  };

  const handleRebalance = async () => {
    try {
      const response = await fetch('/api/v1/bankroll/rebalance', { method: 'POST' });
      if (response.ok) {
        await fetchBankrollState();
      }
    } catch (error) {
      console.error('Failed to rebalance:', error);
    }
  };

  const handleStrategyChange = async (strategy: string) => {
    try {
      const response = await fetch('/api/v1/bankroll/strategy', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ strategy }),
      });
      if (response.ok) {
        setState(prev => ({ ...prev, strategy: strategy as any }));
      }
    } catch (error) {
      console.error('Failed to change strategy:', error);
    }
  };

  const handleAllocate = async (bookmakerId: string, amount: number) => {
    try {
      const response = await fetch(`/api/v1/bankroll/allocate/${bookmakerId}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ amount }),
      });
      if (response.ok) {
        await fetchBankrollState();
        setEditingBookmaker(null);
      }
    } catch (error) {
      console.error('Failed to allocate:', error);
    }
  };

  const formatMoney = (amount: number) => {
    return `${Math.round(amount).toLocaleString('ru-RU')} ₽`;
  };

  const progressToTarget = (state.dailyProfit / state.dailyTarget) * 100;
  const exposurePercent = (state.allocated / state.total) * 100;

  return (
    <div className="bankroll-panel">
      {/* Header */}
      <div className="bankroll-header">
        <div className="header-left">
          <Wallet size={24} className="header-icon" />
          <h2>Управление банкроллом</h2>
        </div>
        <button 
          className="btn-settings"
          onClick={() => setShowSettings(!showSettings)}
        >
          <Settings size={18} />
        </button>
      </div>

      {/* Main Stats */}
      <div className="stats-grid">
        <div className="stat-card total">
          <div className="stat-icon"><Wallet size={20} /></div>
          <div className="stat-content">
            <span className="stat-label">Общий банкролл</span>
            <span className="stat-value">{formatMoney(state.total)}</span>
          </div>
        </div>

        <div className="stat-card available">
          <div className="stat-icon"><RefreshCw size={20} /></div>
          <div className="stat-content">
            <span className="stat-label">Доступно</span>
            <span className="stat-value">{formatMoney(state.available)}</span>
          </div>
        </div>

        <div className="stat-card allocated">
          <div className="stat-icon"><PieChart size={20} /></div>
          <div className="stat-content">
            <span className="stat-label">Задействовано</span>
            <span className="stat-value">{formatMoney(state.allocated)}</span>
            <span className="stat-percent">{exposurePercent.toFixed(1)}%</span>
          </div>
          <div className="exposure-bar">
            <div 
              className={`exposure-fill ${exposurePercent > 30 ? 'warning' : ''}`}
              style={{ width: `${Math.min(exposurePercent, 100)}%` }}
            />
          </div>
        </div>

        <div className="stat-card profit">
          <div className="stat-icon"><TrendingUp size={20} /></div>
          <div className="stat-content">
            <span className="stat-label">Профит сегодня</span>
            <span className="stat-value positive">+{formatMoney(state.dailyProfit)}</span>
          </div>
          <div className="target-progress">
            <div className="target-bar">
              <div 
                className="target-fill"
                style={{ width: `${Math.min(progressToTarget, 100)}%` }}
              />
            </div>
            <span className="target-text">{progressToTarget.toFixed(0)}% от цели</span>
          </div>
        </div>
      </div>

      {/* Strategy Selector */}
      <div className="strategy-section">
        <h3>Стратегия распределения</h3>
        <div className="strategy-buttons">
          <button 
            className={`strategy-btn ${state.strategy === 'equal_profit' ? 'active' : ''}`}
            onClick={() => handleStrategyChange('equal_profit')}
          >
            Равная прибыль
          </button>
          <button 
            className={`strategy-btn ${state.strategy === 'max_volume' ? 'active' : ''}`}
            onClick={() => handleStrategyChange('max_volume')}
          >
            Макс объем
          </button>
          <button 
            className={`strategy-btn ${state.strategy === 'fixed' ? 'active' : ''}`}
            onClick={() => handleStrategyChange('fixed')}
          >
            Фиксированная
          </button>
        </div>
      </div>

      {/* Bookmaker Allocations */}
      <div className="allocations-section">
        <div className="section-header">
          <h3>Распределение по БК</h3>
          <button className="btn-rebalance" onClick={handleRebalance}>
            <ArrowRightLeft size={14} />
            Перебалансировать
          </button>
        </div>

        <div className="allocations-list">
          {allocations.map(alloc => (
            <div key={alloc.id} className={`allocation-card ${alloc.status}`}>
              <div className="allocation-header">
                <span className="bk-name">{alloc.name}</span>
                <span className={`status-badge ${alloc.status}`}>
                  {alloc.status === 'ready' && 'Готов'}
                  {alloc.status === 'low' && 'Мало средств'}
                  {alloc.status === 'empty' && 'Нет средств'}
                </span>
              </div>

              <div className="allocation-stats">
                <div className="alloc-stat">
                  <span className="label">Выделено:</span>
                  <span className="value">{formatMoney(alloc.allocated)}</span>
                </div>
                <div className="alloc-stat">
                  <span className="label">Доступно:</span>
                  <span className="value">{formatMoney(alloc.available)}</span>
                </div>
                <div className="alloc-stat">
                  <span className="label">В работе:</span>
                  <span className="value">{formatMoney(alloc.inUse)}</span>
                </div>
              </div>

              {editingBookmaker === alloc.id ? (
                <div className="allocation-edit">
                  <input 
                    type="number" 
                    defaultValue={alloc.allocated}
                    id={`alloc-input-${alloc.id}`}
                  />
                  <button 
                    className="btn-confirm"
                    onClick={() => {
                      const input = document.getElementById(`alloc-input-${alloc.id}`) as HTMLInputElement;
                      handleAllocate(alloc.id, Number(input.value));
                    }}
                  >
                    <Plus size={14} />
                  </button>
                  <button 
                    className="btn-cancel"
                    onClick={() => setEditingBookmaker(null)}
                  >
                    <Minus size={14} />
                  </button>
                </div>
              ) : (
                <button 
                  className="btn-edit-alloc"
                  onClick={() => setEditingBookmaker(alloc.id)}
                >
                  Изменить
                </button>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Warnings */}
      {exposurePercent > 30 && (
        <div className="warning-banner">
          <AlertTriangle size={16} />
          <span>Высокая экспозиция: {exposurePercent.toFixed(1)}% банкролла задействовано</span>
        </div>
      )}

      {progressToTarget >= 90 && (
        <div className="success-banner">
          <TrendingUp size={16} />
          <span>Цель дня почти достигнута! ({progressToTarget.toFixed(0)}%)</span>
        </div>
      )}

      {/* Settings Modal */}
      <AnimatePresence>
        {showSettings && (
          <BankrollSettingsModal 
            state={state}
            onUpdate={setState}
            onClose={() => setShowSettings(false)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

function BankrollSettingsModal({ 
  state, 
  onUpdate, 
  onClose 
}: { 
  state: BankrollState; 
  onUpdate: (s: BankrollState) => void;
  onClose: () => void;
}) {
  const [localState, setLocalState] = useState(state);

  const handleSave = () => {
    onUpdate(localState);
    onClose();
  };

  return (
    <motion.div 
      className="modal-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={onClose}
    >
      <motion.div 
        className="modal-content bankroll-settings"
        initial={{ scale: 0.9, y: 20 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.9, y: 20 }}
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>Настройки банкролла</h3>
          <button className="btn-close" onClick={onClose}>×</button>
        </div>

        <div className="settings-content">
          <div className="setting-group">
            <label>Общий банкролл (₽)</label>
            <input 
              type="number"
              value={localState.total}
              onChange={e => setLocalState(prev => ({ ...prev, total: Number(e.target.value) }))}
            />
          </div>

          <div className="setting-group">
            <label>Цель прибыли на день (₽)</label>
            <input 
              type="number"
              value={localState.dailyTarget}
              onChange={e => setLocalState(prev => ({ ...prev, dailyTarget: Number(e.target.value) }))}
            />
          </div>

          <div className="setting-group">
            <label>Макс экспозиция (%)</label>
            <input 
              type="number"
              min="1"
              max="100"
              value={localState.maxExposure * 100}
              onChange={e => setLocalState(prev => ({ ...prev, maxExposure: Number(e.target.value) / 100 }))}
            />
          </div>

          <div className="setting-group">
            <label>Макс ставка (₽)</label>
            <input 
              type="number"
              value={50000}
              onChange={() => {}}
            />
          </div>

          <div className="setting-group">
            <label>Мин ставка (₽)</label>
            <input 
              type="number"
              value={100}
              onChange={() => {}}
            />
          </div>

          <div className="setting-group checkbox">
            <label>
              <input type="checkbox" defaultChecked />
              Остановить при достижении цели
            </label>
          </div>

          <div className="setting-group checkbox">
            <label>
              <input type="checkbox" defaultChecked />
              Уведомление при низком балансе
            </label>
          </div>
        </div>

        <div className="modal-actions">
          <button className="btn-secondary" onClick={onClose}>Отмена</button>
          <button className="btn-primary" onClick={handleSave}>Сохранить</button>
        </div>
      </motion.div>
    </motion.div>
  );
}

export default BankrollPanel;
