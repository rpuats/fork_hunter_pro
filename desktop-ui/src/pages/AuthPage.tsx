import { useState, useEffect, useCallback } from 'react';
import { 
  Shield, 
  Plus, 
  Trash2, 
  RefreshCw, 
  CheckCircle2, 
  XCircle, 
  Clock,
  AlertCircle,
  ChevronDown,
  Lock,
  Eye,
  EyeOff,
  LogOut
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

// Types
export interface BookmakerAccount {
  bookmaker_id: string;
  login: string;
  password: string;
  phone_prefix?: string;
  two_fa_secret?: string;
  status: 'NotAuthenticated' | 'AwaitingCaptcha' | 'Awaiting2FA' | 'Authenticated' | 'SessionExpired' | 'AuthFailed';
  balance?: number;
  last_auth?: string;
  error_message?: string;
}

const SUPPORTED_BOOKMAKERS = [
  { id: 'pari', name: 'Пари', icon: '/icons/pari.png' },
  { id: 'fonbet', name: 'Фонбет', icon: '/icons/fonbet.png' },
  { id: 'marathon', name: 'Марафон', icon: '/icons/marathon.png' },
  { id: 'betcity', name: 'Бетсити', icon: '/icons/betcity.png' },
  { id: 'zenit', name: 'Зенит', icon: '/icons/zenit.png' },
  { id: 'baltbet', name: 'Балтбет', icon: '/icons/baltbet.png' },
  { id: 'bettery', name: 'Беттери', icon: '/icons/bettery.png' },
  { id: 'leon', name: 'Леон', icon: '/icons/leon.png' },
  { id: 'sportbet', name: 'Спортбет', icon: '/icons/sportbet.png' },
  { id: 'bet24', name: '24bet', icon: '/icons/bet24.png' },
  { id: 'winline', name: 'Винлайн', icon: '/icons/winline.png' },
  { id: 'olimp', name: 'Олимп', icon: '/icons/olimp.png' },
];

const STATUS_CONFIG: Record<string, { label: string; color: string; icon: React.ReactNode }> = {
  NotAuthenticated: { 
    label: 'Не авторизован', 
    color: '#6e6e7d', 
    icon: <XCircle size={16} /> 
  },
  AwaitingCaptcha: { 
    label: 'Ждёт капчу', 
    color: '#f59e0b', 
    icon: <AlertCircle size={16} /> 
  },
  Awaiting2FA: { 
    label: 'Ждёт 2FA', 
    color: '#f59e0b', 
    icon: <Clock size={16} /> 
  },
  Authenticated: { 
    label: 'Авторизован', 
    color: '#10b981', 
    icon: <CheckCircle2 size={16} /> 
  },
  SessionExpired: { 
    label: 'Сессия истекла', 
    color: '#6e6e7d', 
    icon: <Clock size={16} /> 
  },
  AuthFailed: { 
    label: 'Ошибка авторизации', 
    color: '#ef4444', 
    icon: <XCircle size={16} /> 
  },
};

export function AuthPage() {
  const [accounts, setAccounts] = useState<BookmakerAccount[]>([]);
  const [selectedAccounts, setSelectedAccounts] = useState<Set<string>>(new Set());
  const [showAddModal, setShowAddModal] = useState(false);
  const [showCaptchaModal, setShowCaptchaModal] = useState(false);
  const [show2FAModal, setShow2FAModal] = useState(false);
  const [activeAccount, setActiveAccount] = useState<BookmakerAccount | null>(null);
  const [captchaCode, setCaptchaCode] = useState('');
  const [twoFACode, setTwoFACode] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  // Load accounts from backend
  useEffect(() => {
    fetchAccounts();
  }, []);

  const fetchAccounts = async () => {
    try {
      const response = await fetch('/api/auth/accounts');
      if (response.ok) {
        const data = await response.json();
        setAccounts(data);
      }
    } catch (error) {
      console.error('Failed to fetch accounts:', error);
    }
  };

  const handleAddAccount = async (account: Omit<BookmakerAccount, 'status' | 'balance'>) => {
    try {
      const response = await fetch('/api/auth/accounts', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ...account,
          status: 'NotAuthenticated',
        }),
      });

      if (response.ok) {
        await fetchAccounts();
        setShowAddModal(false);
      }
    } catch (error) {
      console.error('Failed to add account:', error);
    }
  };

  const handleDeleteAccount = async (bookmakerId: string) => {
    if (!confirm(`Удалить аккаунт ${bookmakerId}?`)) return;

    try {
      const response = await fetch(`/api/auth/accounts/${bookmakerId}`, {
        method: 'DELETE',
      });

      if (response.ok) {
        await fetchAccounts();
      }
    } catch (error) {
      console.error('Failed to delete account:', error);
    }
  };

  const handleAuthenticateOne = async (bookmakerId: string) => {
    setIsLoading(true);
    setActiveAccount(accounts.find(a => a.bookmaker_id === bookmakerId) || null);

    try {
      const response = await fetch(`/api/auth/authenticate/${bookmakerId}`, {
        method: 'POST',
      });

      if (!response.ok) {
        const error = await response.json();
        
        if (error.code === 'CAPTCHA_REQUIRED') {
          setShowCaptchaModal(true);
        } else if (error.code === '2FA_REQUIRED') {
          setShow2FAModal(true);
        } else {
          alert(`Ошибка авторизации: ${error.message}`);
        }
      } else {
        await fetchAccounts();
      }
    } catch (error) {
      console.error('Authentication failed:', error);
      alert('Ошибка соединения с сервером');
    } finally {
      setIsLoading(false);
    }
  };

  const handleAuthenticateAll = async () => {
    const selected = Array.from(selectedAccounts);
    if (selected.length === 0) {
      alert('Выберите хотя бы один аккаунт');
      return;
    }

    setIsLoading(true);
    try {
      const response = await fetch('/api/auth/authenticate-all', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ bookmakers: selected }),
      });

      if (response.ok) {
        await fetchAccounts();
      } else {
        const error = await response.json();
        alert(`Ошибка: ${error.message}`);
      }
    } catch (error) {
      console.error('Batch authentication failed:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const handleSubmitCaptcha = async () => {
    if (!activeAccount) return;

    try {
      const response = await fetch(`/api/auth/captcha/${activeAccount.bookmaker_id}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: captchaCode }),
      });

      if (response.ok) {
        setShowCaptchaModal(false);
        setCaptchaCode('');
        await fetchAccounts();
      } else {
        const error = await response.json();
        alert(`Ошибка капчи: ${error.message}`);
      }
    } catch (error) {
      console.error('Failed to submit captcha:', error);
    }
  };

  const handleSubmit2FA = async () => {
    if (!activeAccount) return;

    try {
      const response = await fetch(`/api/auth/2fa/${activeAccount.bookmaker_id}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: twoFACode }),
      });

      if (response.ok) {
        setShow2FAModal(false);
        setTwoFACode('');
        await fetchAccounts();
      } else {
        const error = await response.json();
        alert(`Ошибка 2FA: ${error.message}`);
      }
    } catch (error) {
      console.error('Failed to submit 2FA:', error);
    }
  };

  const handleRefreshBalance = async (bookmakerId: string) => {
    try {
      const response = await fetch(`/api/auth/balance/${bookmakerId}`, {
        method: 'GET',
      });

      if (response.ok) {
        await fetchAccounts();
      }
    } catch (error) {
      console.error('Failed to refresh balance:', error);
    }
  };

  const handleLogout = async (bookmakerId: string) => {
    try {
      const response = await fetch(`/api/auth/logout/${bookmakerId}`, {
        method: 'POST',
      });

      if (response.ok) {
        await fetchAccounts();
      }
    } catch (error) {
      console.error('Failed to logout:', error);
    }
  };

  const toggleSelection = (bookmakerId: string) => {
    const newSet = new Set(selectedAccounts);
    if (newSet.has(bookmakerId)) {
      newSet.delete(bookmakerId);
    } else {
      newSet.add(bookmakerId);
    }
    setSelectedAccounts(newSet);
  };

  const selectAll = () => {
    const allNonAuth = accounts
      .filter(a => a.status !== 'Authenticated')
      .map(a => a.bookmaker_id);
    setSelectedAccounts(new Set(allNonAuth));
  };

  const formatMoney = (amount?: number) => {
    if (amount === undefined) return '—';
    return `${Math.round(amount).toLocaleString('ru-RU')} ₽`;
  };

  const formatDate = (date?: string) => {
    if (!date) return 'Никогда';
    return new Date(date).toLocaleString('ru-RU');
  };

  const maskLogin = (login: string) => {
    if (login.length <= 6) return login;
    return login.slice(0, 3) + '***' + login.slice(-3);
  };

  return (
    <div className="auth-page">
      {/* Header */}
      <div className="auth-header">
        <div className="header-left">
          <Shield size={24} className="header-icon" />
          <h1>Управление аккаунтами БК</h1>
          <span className="account-count">
            {accounts.filter(a => a.status === 'Authenticated').length} / {accounts.length} авторизовано
          </span>
        </div>
        
        <div className="header-actions">
          <button 
            className="btn-secondary"
            onClick={selectAll}
            disabled={accounts.length === 0}
          >
            Выбрать все неавторизованные
          </button>
          
          <button 
            className="btn-primary"
            onClick={handleAuthenticateAll}
            disabled={selectedAccounts.size === 0 || isLoading}
          >
            {isLoading ? <RefreshCw size={16} className="spin" /> : <CheckCircle2 size={16} />}
            Авторизовать выбранные ({selectedAccounts.size})
          </button>
          
          <button 
            className="btn-success"
            onClick={() => setShowAddModal(true)}
          >
            <Plus size={16} />
            Добавить БК
          </button>
        </div>
      </div>

      {/* Accounts Table */}
      <div className="accounts-table-container">
        <table className="accounts-table">
          <thead>
            <tr>
              <th style={{ width: 40 }}>
                <input 
                  type="checkbox" 
                  checked={selectedAccounts.size > 0 && selectedAccounts.size === accounts.filter(a => a.status !== 'Authenticated').length}
                  onChange={() => {}}
                />
              </th>
              <th>Букмекер</th>
              <th>Логин</th>
              <th>Статус</th>
              <th>Баланс</th>
              <th>Последняя авторизация</th>
              <th>Действия</th>
            </tr>
          </thead>
          <tbody>
            {accounts.length === 0 ? (
              <tr>
                <td colSpan={7} className="empty-state">
                  <div className="empty-content">
                    <Shield size={48} className="empty-icon" />
                    <p>Нет добавленных аккаунтов</p>
                    <button className="btn-primary" onClick={() => setShowAddModal(true)}>
                      Добавить первый аккаунт
                    </button>
                  </div>
                </td>
              </tr>
            ) : (
              accounts.map(account => {
                const statusConfig = STATUS_CONFIG[account.status] || STATUS_CONFIG.NotAuthenticated;
                const bkInfo = SUPPORTED_BOOKMAKERS.find(b => b.id === account.bookmaker_id);
                
                return (
                  <tr 
                    key={account.bookmaker_id}
                    className={account.status.toLowerCase()}
                  >
                    <td>
                      {account.status !== 'Authenticated' && (
                        <input 
                          type="checkbox"
                          checked={selectedAccounts.has(account.bookmaker_id)}
                          onChange={() => toggleSelection(account.bookmaker_id)}
                        />
                      )}
                    </td>
                    <td className="bk-cell">
                      <img 
                        src={bkInfo?.icon || '/icons/default-bk.png'} 
                        alt={bkInfo?.name || account.bookmaker_id}
                        className="bk-icon"
                        onError={(e) => {
                          (e.target as HTMLImageElement).style.display = 'none';
                        }}
                      />
                      <span className="bk-name">{bkInfo?.name || account.bookmaker_id}</span>
                    </td>
                    <td className="login-cell">
                      <span className="login-text">{maskLogin(account.login)}</span>
                    </td>
                    <td>
                      <span 
                        className="status-badge"
                        style={{ color: statusConfig.color }}
                      >
                        {statusConfig.icon}
                        {statusConfig.label}
                      </span>
                      {account.error_message && (
                        <span className="error-hint" title={account.error_message}>
                          <AlertCircle size={14} />
                        </span>
                      )}
                    </td>
                    <td className="balance-cell">
                      {account.status === 'Authenticated' ? (
                        <span className="balance-value">{formatMoney(account.balance)}</span>
                      ) : (
                        <span className="balance-placeholder">—</span>
                      )}
                    </td>
                    <td className="date-cell">{formatDate(account.last_auth)}</td>
                    <td className="actions-cell">
                      {account.status === 'NotAuthenticated' || account.status === 'SessionExpired' || account.status === 'AuthFailed' ? (
                        <button 
                          className="btn-action btn-auth"
                          onClick={() => handleAuthenticateOne(account.bookmaker_id)}
                          disabled={isLoading}
                        >
                          {isLoading && activeAccount?.bookmaker_id === account.bookmaker_id ? (
                            <RefreshCw size={14} className="spin" />
                          ) : (
                            <Lock size={14} />
                          )}
                          Авторизоваться
                        </button>
                      ) : account.status === 'AwaitingCaptcha' ? (
                        <button 
                          className="btn-action btn-captcha"
                          onClick={() => {
                            setActiveAccount(account);
                            setShowCaptchaModal(true);
                          }}
                        >
                          <AlertCircle size={14} />
                          Ввести капчу
                        </button>
                      ) : account.status === 'Awaiting2FA' ? (
                        <button 
                          className="btn-action btn-2fa"
                          onClick={() => {
                            setActiveAccount(account);
                            setShow2FAModal(true);
                          }}
                        >
                          <Clock size={14} />
                          Ввести код 2FA
                        </button>
                      ) : (
                        <>
                          <button 
                            className="btn-action btn-refresh"
                            onClick={() => handleRefreshBalance(account.bookmaker_id)}
                            title="Обновить баланс"
                          >
                            <RefreshCw size={14} />
                          </button>
                          <button 
                            className="btn-action btn-logout"
                            onClick={() => handleLogout(account.bookmaker_id)}
                            title="Выйти"
                          >
                            <LogOut size={14} />
                          </button>
                        </>
                      )}
                      
                      <button 
                        className="btn-action btn-delete"
                        onClick={() => handleDeleteAccount(account.bookmaker_id)}
                        title="Удалить аккаунт"
                      >
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {/* Add Account Modal */}
      <AnimatePresence>
        {showAddModal && (
          <AddAccountModal 
            onClose={() => setShowAddModal(false)}
            onAdd={handleAddAccount}
          />
        )}
      </AnimatePresence>

      {/* Captcha Modal */}
      <AnimatePresence>
        {showCaptchaModal && activeAccount && (
          <CaptchaModal
            bookmakerName={activeAccount.bookmaker_id}
            onClose={() => setShowCaptchaModal(false)}
            onSubmit={handleSubmitCaptcha}
            captchaCode={captchaCode}
            setCaptchaCode={setCaptchaCode}
          />
        )}
      </AnimatePresence>

      {/* 2FA Modal */}
      <AnimatePresence>
        {show2FAModal && activeAccount && (
          <TwoFAModal
            bookmakerName={activeAccount.bookmaker_id}
            onClose={() => setShow2FAModal(false)}
            onSubmit={handleSubmit2FA}
            twoFACode={twoFACode}
            setTwoFACode={setTwoFACode}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

// Sub-components
function AddAccountModal({ 
  onClose, 
  onAdd 
}: { 
  onClose: () => void; 
  onAdd: (account: any) => void;
}) {
  const [selectedBK, setSelectedBK] = useState('');
  const [login, setLogin] = useState('');
  const [password, setPassword] = useState('');
  const [phonePrefix, setPhonePrefix] = useState('+7');
  const [has2FA, setHas2FA] = useState(false);
  const [twoFASecret, setTwoFASecret] = useState('');
  const [showPassword, setShowPassword] = useState(false);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onAdd({
      bookmaker_id: selectedBK,
      login,
      password,
      phone_prefix: login.match(/^\d/) ? phonePrefix : undefined,
      two_fa_secret: has2FA ? twoFASecret : undefined,
    });
  };

  const isPhone = login.match(/^\d/);

  return (
    <motion.div 
      className="modal-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={onClose}
    >
      <motion.div 
        className="modal-content"
        initial={{ scale: 0.9, y: 20 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.9, y: 20 }}
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>Добавить аккаунт букмекера</h3>
          <button className="btn-close" onClick={onClose}>×</button>
        </div>

        <form onSubmit={handleSubmit} className="modal-form">
          <div className="form-group">
            <label>Букмекер</label>
            <div className="select-wrapper">
              <select 
                value={selectedBK} 
                onChange={e => setSelectedBK(e.target.value)}
                required
              >
                <option value="">Выберите букмекера</option>
                {SUPPORTED_BOOKMAKERS.map(bk => (
                  <option key={bk.id} value={bk.id}>{bk.name}</option>
                ))}
              </select>
              <ChevronDown size={16} className="select-icon" />
            </div>
          </div>

          <div className="form-group">
            <label>Логин / Телефон / Email</label>
            <div className="input-with-prefix">
              {isPhone && (
                <select 
                  value={phonePrefix}
                  onChange={e => setPhonePrefix(e.target.value)}
                  className="prefix-select"
                >
                  <option value="+7">+7</option>
                  <option value="+375">+375</option>
                </select>
              )}
              <input 
                type="text"
                value={login}
                onChange={e => setLogin(e.target.value)}
                placeholder={isPhone ? '9991234567' : 'login@email.com'}
                required
              />
            </div>
            {isPhone && (
              <span className="input-hint">Номер телефона без +7, оно добавится автоматически</span>
            )}
          </div>

          <div className="form-group">
            <label>Пароль</label>
            <div className="password-input-wrapper">
              <input 
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={e => setPassword(e.target.value)}
                required
              />
              <button 
                type="button" 
                className="btn-toggle-password"
                onClick={() => setShowPassword(!showPassword)}
              >
                {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
              </button>
            </div>
          </div>

          <div className="form-group checkbox">
            <label>
              <input 
                type="checkbox"
                checked={has2FA}
                onChange={e => setHas2FA(e.target.checked)}
              />
              Есть 2FA (двухфакторная аутентификация)
            </label>
          </div>

          {has2FA && (
            <div className="form-group">
              <label>TOTP Secret (опционально)</label>
              <input 
                type="text"
                value={twoFASecret}
                onChange={e => setTwoFASecret(e.target.value)}
                placeholder="JBSWY3DPEHPK3PXP"
              />
              <span className="input-hint">Если указан — коды 2FA будут генерироваться автоматически</span>
            </div>
          )}

          <div className="modal-actions">
            <button type="button" className="btn-secondary" onClick={onClose}>
              Отмена
            </button>
            <button 
              type="submit" 
              className="btn-primary"
              disabled={!selectedBK || !login || !password}
            >
              <Plus size={16} />
              Добавить аккаунт
            </button>
          </div>
        </form>
      </motion.div>
    </motion.div>
  );
}

function CaptchaModal({ 
  bookmakerName, 
  onClose, 
  onSubmit,
  captchaCode,
  setCaptchaCode
}: { 
  bookmakerName: string;
  onClose: () => void;
  onSubmit: () => void;
  captchaCode: string;
  setCaptchaCode: (code: string) => void;
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
        className="modal-content captcha-modal"
        initial={{ scale: 0.9, y: 20 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.9, y: 20 }}
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>Ввод капчи — {bookmakerName}</h3>
          <button className="btn-close" onClick={onClose}>×</button>
        </div>

        <div className="captcha-content">
          <p className="captcha-instructions">
            Браузер открыл страницу входа. Введите символы с картинки:
          </p>
          
          {/* Placeholder for captcha image */}
          <div className="captcha-image-placeholder">
            <AlertCircle size={48} />
            <span>Капча отображается в браузере</span>
          </div>

          <div className="form-group">
            <input 
              type="text"
              value={captchaCode}
              onChange={e => setCaptchaCode(e.target.value)}
              placeholder="Введите код с картинки"
              autoFocus
              className="captcha-input"
            />
          </div>
        </div>

        <div className="modal-actions">
          <button type="button" className="btn-secondary" onClick={onClose}>
            Отмена
          </button>
          <button 
            type="button" 
            className="btn-primary"
            onClick={onSubmit}
            disabled={!captchaCode}
          >
            <CheckCircle2 size={16} />
            Подтвердить
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

function TwoFAModal({ 
  bookmakerName, 
  onClose, 
  onSubmit,
  twoFACode,
  setTwoFACode
}: { 
  bookmakerName: string;
  onClose: () => void;
  onSubmit: () => void;
  twoFACode: string;
  setTwoFACode: (code: string) => void;
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
        className="modal-content twofa-modal"
        initial={{ scale: 0.9, y: 20 }}
        animate={{ scale: 1, y: 0 }}
        exit={{ scale: 0.9, y: 20 }}
        onClick={e => e.stopPropagation()}
      >
        <div className="modal-header">
          <h3>Двухфакторная аутентификация — {bookmakerName}</h3>
          <button className="btn-close" onClick={onClose}>×</button>
        </div>

        <div className="twofa-content">
          <p className="twofa-instructions">
            Введите код подтверждения из SMS, email или приложения аутентификатора:
          </p>

          <div className="form-group">
            <input 
              type="text"
              value={twoFACode}
              onChange={e => setTwoFACode(e.target.value.replace(/\D/g, '').slice(0, 6))}
              placeholder="123456"
              autoFocus
              className="twofa-input"
              maxLength={6}
            />
          </div>

          <p className="twofa-hint">
            Код действителен в течение 30-60 секунд
          </p>
        </div>

        <div className="modal-actions">
          <button type="button" className="btn-secondary" onClick={onClose}>
            Отмена
          </button>
          <button 
            type="button" 
            className="btn-primary"
            onClick={onSubmit}
            disabled={twoFACode.length < 4}
          >
            <CheckCircle2 size={16} />
            Подтвердить
          </button>
        </div>
      </motion.div>
    </motion.div>
  );
}

export default AuthPage;
