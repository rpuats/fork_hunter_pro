import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { 
  Plus, RefreshCw, AlertTriangle, CheckCircle2, XCircle, Clock,
  ChevronRight, ChevronDown, Settings, X, Copy, Trash2, Fingerprint,
  Globe, Shield, User, Monitor, Cpu, HardDrive, Calendar,
  Wifi, WifiOff, Cookie, KeyRound, ExternalLink
} from 'lucide-react'
import { demoProfiles, type BookmakerProfileGroup, type AccountProfile, switchProfile, createProfile, deleteProfile } from '../lib/profiles'
import { demoAccounts } from '../lib/demoData'

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

const formatMoney = (amount: number) => new Intl.NumberFormat('ru-RU').format(amount) + ' ₽'

const formatRelativeTime = (dateStr: string | null) => {
  if (!dateStr) return 'никогда'
  const diff = Math.floor((Date.now() - new Date(dateStr).getTime()) / 60000)
  if (diff < 1) return 'только что'
  if (diff < 60) return `${diff} мин назад`
  const hours = Math.floor(diff / 60)
  if (hours < 24) return `${hours} ч назад`
  return `${Math.floor(hours / 24)} дн назад`
}

// Bookmaker colors
const bkColors: Record<string, string> = {
  Pari: 'bg-indigo-500',
  Fonbet: 'bg-emerald-500',
  Leon: 'bg-amber-500',
  Winline: 'bg-red-500',
  Olimp: 'bg-blue-500'
}

export function AccountsPage() {
  const [profileGroups, setProfileGroups] = useState<BookmakerProfileGroup[]>(demoProfiles)
  const [expandedBookmaker, setExpandedBookmaker] = useState<string | null>('Pari')
  const [showCreateModal, setShowCreateModal] = useState(false)
  const [showProfileDetail, setShowProfileDetail] = useState<AccountProfile | null>(null)
  const [newProfileName, setNewProfileName] = useState('')
  const [selectedBookmakerForCreate, setSelectedBookmakerForCreate] = useState('')
  const [copied, setCopied] = useState(false)

  const handleSwitchProfile = (bookmaker: string, profileId: string) => {
    setProfileGroups(prev => prev.map(g => 
      g.bookmaker === bookmaker ? switchProfile(g, profileId) : g
    ))
  }

  const handleCreateProfile = () => {
    if (!newProfileName || !selectedBookmakerForCreate) return
    setProfileGroups(prev => prev.map(g => 
      g.bookmaker === selectedBookmakerForCreate ? createProfile(g, newProfileName) : g
    ))
    setNewProfileName('')
    setShowCreateModal(false)
  }

  const handleDeleteProfile = (bookmaker: string, profileId: string) => {
    if (!confirm('Удалить профиль? Данные авторизации будут потеряны.')) return
    setProfileGroups(prev => prev.map(g => 
      g.bookmaker === bookmaker ? deleteProfile(g, profileId) : g
    ))
    if (showProfileDetail?.id === profileId) setShowProfileDetail(null)
  }

  const activeProfiles = profileGroups.map(g => g.profiles.find(p => p.isActive)).filter(Boolean)
  const totalProfiles = profileGroups.reduce((sum, g) => sum + g.profiles.length, 0)

  // Demo accounts lookup
  const getAccountBalance = (bookmaker: string) => {
    const acc = demoAccounts.find(a => a.name === bookmaker)
    return acc ? acc.balance : 0
  }

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 space-y-6">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">Профили и аккаунты</h1>
          <p className="text-sm text-text-secondary mt-1">
            {totalProfiles} профилей • {activeProfiles.length} активных • Смена дропа в один клик
          </p>
        </div>
        <button 
          onClick={() => {
            setSelectedBookmakerForCreate(profileGroups[0].bookmaker)
            setShowCreateModal(true)
          }}
          className="btn btn-primary text-sm flex items-center gap-2"
        >
          <Plus size={16} /> Новый профиль
        </button>
      </motion.div>

      {/* Quick Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-4 gap-4">
        <motion.div variants={item} className="rounded-card border border-border bg-surface p-4">
          <div className="text-sm text-text-secondary">Всего профилей</div>
          <div className="text-2xl font-bold text-text-primary">{totalProfiles}</div>
        </motion.div>
        <motion.div variants={item} className="rounded-card border border-border bg-surface p-4">
          <div className="text-sm text-text-secondary">Активных</div>
          <div className="text-2xl font-bold text-emerald-400">{activeProfiles.length}</div>
        </motion.div>
        <motion.div variants={item} className="rounded-card border border-border bg-surface p-4">
          <div className="text-sm text-text-secondary">С прокси</div>
          <div className="text-2xl font-bold text-blue-400">
            {profileGroups.flatMap(g => g.profiles).filter(p => p.proxy).length}
          </div>
        </motion.div>
        <motion.div variants={item} className="rounded-card border border-border bg-surface p-4">
          <div className="text-sm text-text-secondary">Уникальных отпечатков</div>
          <div className="text-2xl font-bold text-purple-400">{totalProfiles}</div>
        </motion.div>
      </div>

      {/* Bookmaker Cards with Profiles */}
      <motion.div variants={item} className="space-y-3">
        {profileGroups.map(group => {
          const activeProfile = group.profiles.find(p => p.isActive)
          const isExpanded = expandedBookmaker === group.bookmaker
          const hasProfiles = group.profiles.length > 0

          return (
            <div 
              key={group.bookmaker}
              className="rounded-card border border-border bg-surface overflow-hidden"
            >
              {/* Bookmaker Header */}
              <div 
                className="flex items-center gap-4 p-4 cursor-pointer hover:bg-elevated/20 transition-colors"
                onClick={() => setExpandedBookmaker(isExpanded ? null : group.bookmaker)}
              >
                {/* Logo */}
                <div className={`w-12 h-12 rounded-xl flex items-center justify-center text-white font-bold text-lg shrink-0 ${bkColors[group.bookmaker] || 'bg-gray-500'}`}>
                  {group.logo}
                </div>

                {/* Info */}
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <h3 className="font-semibold text-text-primary">{group.bookmaker}</h3>
                    <span className="text-xs px-2 py-0.5 rounded-full bg-background text-text-secondary">
                      {group.profiles.length} профилей
                    </span>
                  </div>
                  {activeProfile ? (
                    <div className="flex items-center gap-2 mt-1">
                      <div className="w-2 h-2 rounded-full bg-emerald-500" />
                      <span className="text-sm text-text-secondary">
                        Активен: <span style={{ color: activeProfile.color }}>{activeProfile.name}</span>
                      </span>
                      {activeProfile.proxy && (
                        <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20">
                          {activeProfile.proxy.type}
                        </span>
                      )}
                    </div>
                  ) : (
                    <div className="flex items-center gap-2 mt-1">
                      <div className="w-2 h-2 rounded-full bg-gray-500" />
                      <span className="text-sm text-text-muted">Нет профилей</span>
                    </div>
                  )}
                </div>

                {/* Balance */}
                {hasProfiles && (
                  <div className="text-right shrink-0 hidden sm:block">
                    <div className="text-sm font-medium text-text-primary">
                      {formatMoney(getAccountBalance(group.bookmaker))}
                    </div>
                    <div className="text-xs text-text-secondary">Баланс</div>
                  </div>
                )}

                {/* Expand */}
                <ChevronDown 
                  size={20} 
                  className={`text-text-muted transition-transform shrink-0 ${isExpanded ? 'rotate-180' : ''}`}
                />
              </div>

              {/* Profiles List */}
              <AnimatePresence>
                {isExpanded && (
                  <motion.div
                    initial={{ height: 0, opacity: 0 }}
                    animate={{ height: 'auto', opacity: 1 }}
                    exit={{ height: 0, opacity: 0 }}
                    transition={{ duration: 0.2 }}
                    className="overflow-hidden"
                  >
                    <div className="px-4 pb-4 space-y-2">
                      {group.profiles.length === 0 ? (
                        <div className="text-center py-6 text-text-muted">
                          <User size={32} className="mx-auto opacity-20 mb-2" />
                          <p className="text-sm">Нет профилей</p>
                          <button 
                            onClick={(e) => {
                              e.stopPropagation()
                              setSelectedBookmakerForCreate(group.bookmaker)
                              setShowCreateModal(true)
                            }}
                            className="text-xs text-accent hover:text-accent-hover mt-2"
                          >
                            Создать первый профиль
                          </button>
                        </div>
                      ) : (
                        group.profiles.map(profile => (
                          <div 
                            key={profile.id}
                            className={`flex items-center gap-3 p-3 rounded-lg border transition-all ${
                              profile.isActive 
                                ? 'bg-accent/5 border-accent/30' 
                                : 'bg-background border-transparent hover:border-border'
                            }`}
                          >
                            {/* Active indicator */}
                            <button
                              onClick={(e) => {
                                e.stopPropagation()
                                handleSwitchProfile(group.bookmaker, profile.id)
                              }}
                              className={`w-5 h-5 rounded-full border-2 flex items-center justify-center transition-colors ${
                                profile.isActive 
                                  ? 'border-emerald-500 bg-emerald-500' 
                                  : 'border-gray-600 hover:border-gray-400'
                              }`}
                            >
                              {profile.isActive && <CheckCircle2 size={12} className="text-white" />}
                            </button>

                            {/* Profile info */}
                            <div 
                              className="flex-1 min-w-0 cursor-pointer"
                              onClick={() => setShowProfileDetail(profile)}
                            >
                              <div className="flex items-center gap-2">
                                <div 
                                  className="w-3 h-3 rounded-full"
                                  style={{ backgroundColor: profile.color }}
                                />
                                <span className="text-sm font-medium text-text-primary">{profile.name}</span>
                                {profile.isActive && (
                                  <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-emerald-500/20 text-emerald-400">
                                    Активен
                                  </span>
                                )}
                              </div>
                              <div className="flex items-center gap-3 mt-1">
                                <span className="text-xs text-text-muted">{formatRelativeTime(profile.lastUsed)}</span>
                                {profile.proxy && (
                                  <span className="text-xs flex items-center gap-1 text-blue-400">
                                    <Globe size={10} /> {profile.proxy.host}
                                  </span>
                                )}
                                <span className="text-xs flex items-center gap-1 text-purple-400">
                                  <Fingerprint size={10} /> {profile.fingerprint.screenResolution}
                                </span>
                              </div>
                            </div>

                            {/* Actions */}
                            <div className="flex items-center gap-1 shrink-0">
                              <button
                                onClick={(e) => {
                                  e.stopPropagation()
                                  setShowProfileDetail(profile)
                                }}
                                className="p-1.5 rounded hover:bg-white/5 text-text-muted hover:text-text-primary transition-colors"
                                title="Детали"
                              >
                                <Settings size={14} />
                              </button>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation()
                                  const newGroup = createProfile(group, `${profile.name} (копия)`)
                                  setProfileGroups(prev => prev.map(g => g.bookmaker === group.bookmaker ? newGroup : g))
                                }}
                                className="p-1.5 rounded hover:bg-white/5 text-text-muted hover:text-text-primary transition-colors"
                                title="Клонировать"
                              >
                                <Copy size={14} />
                              </button>
                              <button
                                onClick={(e) => {
                                  e.stopPropagation()
                                  handleDeleteProfile(group.bookmaker, profile.id)
                                }}
                                className="p-1.5 rounded hover:bg-white/5 text-text-muted hover:text-red-400 transition-colors"
                                title="Удалить"
                              >
                                <Trash2 size={14} />
                              </button>
                            </div>
                          </div>
                        ))
                      )}

                      {/* Add profile button */}
                      {group.profiles.length > 0 && (
                        <button
                          onClick={(e) => {
                            e.stopPropagation()
                            setSelectedBookmakerForCreate(group.bookmaker)
                            setShowCreateModal(true)
                          }}
                          className="w-full flex items-center justify-center gap-2 py-2 rounded-lg border border-dashed border-border hover:border-accent/50 text-text-muted hover:text-accent transition-colors text-sm"
                        >
                          <Plus size={14} /> Добавить профиль
                        </button>
                      )}
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          )
        })}
      </motion.div>

      {/* Create Profile Modal */}
      <AnimatePresence>
        {showCreateModal && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70"
            onClick={() => setShowCreateModal(false)}
          >
            <motion.div
              initial={{ scale: 0.95, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.95, opacity: 0 }}
              className="w-full max-w-md rounded-2xl bg-surface border border-border p-6"
              onClick={e => e.stopPropagation()}
            >
              <h3 className="text-lg font-semibold text-text-primary mb-4">Новый профиль</h3>
              
              <div className="space-y-4">
                <div>
                  <label className="text-sm text-text-secondary mb-1 block">Букмекер</label>
                  <select 
                    value={selectedBookmakerForCreate}
                    onChange={e => setSelectedBookmakerForCreate(e.target.value)}
                    className="input w-full"
                  >
                    {profileGroups.map(g => (
                      <option key={g.bookmaker}>{g.bookmaker}</option>
                    ))}
                  </select>
                </div>
                
                <div>
                  <label className="text-sm text-text-secondary mb-1 block">Название профиля</label>
                  <input
                    type="text"
                    value={newProfileName}
                    onChange={e => setNewProfileName(e.target.value)}
                    placeholder="Например: Основной, Дроп #2..."
                    className="input w-full"
                    onKeyDown={e => e.key === 'Enter' && handleCreateProfile()}
                  />
                </div>

                <div className="p-3 rounded-lg bg-background">
                  <div className="text-xs text-text-secondary mb-2">Будет создан с уникальным fingerprint:</div>
                  <div className="space-y-1 text-xs text-text-muted">
                    <div className="flex items-center gap-2">
                      <Monitor size={12} /> Случайное разрешение экрана
                    </div>
                    <div className="flex items-center gap-2">
                      <Globe size={12} /> Случайный часовой пояс
                    </div>
                    <div className="flex items-center gap-2">
                      <Cpu size={12} /> Случайное кол-во ядер CPU
                    </div>
                    <div className="flex items-center gap-2">
                      <HardDrive size={12} /> Случайный объем RAM
                    </div>
                  </div>
                </div>
              </div>

              <div className="flex gap-3 mt-6">
                <button 
                  onClick={() => setShowCreateModal(false)}
                  className="btn btn-secondary flex-1 text-sm"
                >
                  Отмена
                </button>
                <button 
                  onClick={handleCreateProfile}
                  disabled={!newProfileName}
                  className="btn btn-primary flex-1 text-sm disabled:opacity-50"
                >
                  Создать
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Profile Detail Modal */}
      <AnimatePresence>
        {showProfileDetail && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70"
            onClick={() => setShowProfileDetail(null)}
          >
            <motion.div
              initial={{ scale: 0.95, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.95, opacity: 0 }}
              className="w-full max-w-lg max-h-[90vh] overflow-auto rounded-2xl bg-surface border border-border"
              onClick={e => e.stopPropagation()}
            >
              {/* Header */}
              <div className="p-5 border-b border-border">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <div 
                      className="w-10 h-10 rounded-lg"
                      style={{ backgroundColor: showProfileDetail.color }}
                    />
                    <div>
                      <h3 className="font-semibold text-text-primary">{showProfileDetail.name}</h3>
                      <p className="text-xs text-text-secondary">{showProfileDetail.bookmaker}</p>
                    </div>
                  </div>
                  <button 
                    onClick={() => setShowProfileDetail(null)}
                    className="p-2 rounded-lg hover:bg-white/5 text-text-muted"
                  >
                    <X size={18} />
                  </button>
                </div>
              </div>

              {/* Body */}
              <div className="p-5 space-y-5">
                {/* Auth Section */}
                <div>
                  <h4 className="text-sm font-medium text-text-secondary mb-3 flex items-center gap-2">
                    <KeyRound size={14} /> Авторизация
                  </h4>
                  <div className="space-y-2">
                    <button className="w-full flex items-center gap-3 p-3 rounded-lg bg-background hover:bg-elevated/50 transition-colors text-left">
                      <Cookie size={16} className="text-amber-400" />
                      <div>
                        <div className="text-sm text-text-primary">Импорт cookies</div>
                        <div className="text-xs text-text-secondary">Вставьте cookies из браузера</div>
                      </div>
                    </button>
                    <button className="w-full flex items-center gap-3 p-3 rounded-lg bg-background hover:bg-elevated/50 transition-colors text-left">
                      <ExternalLink size={16} className="text-blue-400" />
                      <div>
                        <div className="text-sm text-text-primary">Авторизация через браузер</div>
                        <div className="text-xs text-text-secondary">Открыть встроенный браузер</div>
                      </div>
                    </button>
                  </div>
                </div>

                {/* Fingerprint */}
                <div>
                  <h4 className="text-sm font-medium text-text-secondary mb-3 flex items-center gap-2">
                    <Fingerprint size={14} /> Отпечаток браузера
                  </h4>
                  <div className="grid grid-cols-2 gap-2 text-xs">
                    <div className="p-2 rounded bg-background">
                      <div className="text-text-muted">Разрешение</div>
                      <div className="text-text-primary font-medium">{showProfileDetail.fingerprint.screenResolution}</div>
                    </div>
                    <div className="p-2 rounded bg-background">
                      <div className="text-text-muted">Часовой пояс</div>
                      <div className="text-text-primary font-medium">{showProfileDetail.fingerprint.timezone}</div>
                    </div>
                    <div className="p-2 rounded bg-background">
                      <div className="text-text-muted">Ядра CPU</div>
                      <div className="text-text-primary font-medium">{showProfileDetail.fingerprint.cpuCores}</div>
                    </div>
                    <div className="p-2 rounded bg-background">
                      <div className="text-text-muted">RAM</div>
                      <div className="text-text-primary font-medium">{showProfileDetail.fingerprint.memory} GB</div>
                    </div>
                    <div className="p-2 rounded bg-background">
                      <div className="text-text-muted">Язык</div>
                      <div className="text-text-primary font-medium">{showProfileDetail.fingerprint.language}</div>
                    </div>
                    <div className="p-2 rounded bg-background">
                      <div className="text-text-muted">Платформа</div>
                      <div className="text-text-primary font-medium">{showProfileDetail.fingerprint.platform}</div>
                    </div>
                  </div>
                  <button className="mt-2 text-xs text-accent hover:text-accent-hover flex items-center gap-1">
                    <RefreshCw size={12} /> Сгенерировать новый fingerprint
                  </button>
                </div>

                {/* Proxy */}
                <div>
                  <h4 className="text-sm font-medium text-text-secondary mb-3 flex items-center gap-2">
                    <Globe size={14} /> Прокси
                  </h4>
                  {showProfileDetail.proxy ? (
                    <div className="p-3 rounded-lg bg-background space-y-2">
                      <div className="flex justify-between text-sm">
                        <span className="text-text-secondary">Тип:</span>
                        <span className="text-text-primary uppercase">{showProfileDetail.proxy.type}</span>
                      </div>
                      <div className="flex justify-between text-sm">
                        <span className="text-text-secondary">Хост:</span>
                        <span className="text-text-primary">{showProfileDetail.proxy.host}:{showProfileDetail.proxy.port}</span>
                      </div>
                      <button className="text-xs text-accent hover:text-accent-hover mt-2">
                        Изменить прокси
                      </button>
                    </div>
                  ) : (
                    <button className="w-full p-3 rounded-lg border border-dashed border-border hover:border-accent/50 text-text-muted hover:text-accent transition-colors text-sm text-center">
                      + Добавить прокси
                    </button>
                  )}
                </div>

                {/* Info */}
                <div className="grid grid-cols-2 gap-3 text-xs">
                  <div className="p-2 rounded bg-background">
                    <div className="text-text-muted">Создан</div>
                    <div className="text-text-primary">{new Date(showProfileDetail.createdAt).toLocaleDateString('ru-RU')}</div>
                  </div>
                  <div className="p-2 rounded bg-background">
                    <div className="text-text-muted">Последнее использование</div>
                    <div className="text-text-primary">{formatRelativeTime(showProfileDetail.lastUsed)}</div>
                  </div>
                </div>
              </div>

              {/* Footer */}
              <div className="p-5 border-t border-border flex gap-3">
                <button 
                  onClick={() => {
                    const group = profileGroups.find(g => g.bookmaker === showProfileDetail.bookmaker)
                    if (group) {
                      handleSwitchProfile(group.bookmaker, showProfileDetail.id)
                    }
                    setShowProfileDetail(null)
                  }}
                  className="btn btn-primary flex-1 text-sm"
                >
                  Активировать
                </button>
                <button 
                  onClick={() => {
                    handleDeleteProfile(showProfileDetail.bookmaker, showProfileDetail.id)
                    setShowProfileDetail(null)
                  }}
                  className="btn btn-danger flex-1 text-sm"
                >
                  <Trash2 size={14} className="inline mr-1" /> Удалить
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  )
}
