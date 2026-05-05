import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { 
  Settings, Bell, Shield, Database, Palette, Keyboard,
  User, Globe, Moon, Sun, Save, RotateCcw, Check,
  ChevronRight, AlertTriangle, FileText, Download, Trash2
} from 'lucide-react'

const container = { hidden: { opacity: 0 }, show: { opacity: 1, transition: { staggerChildren: 0.05 } } }
const item = { hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0, transition: { duration: 0.3 } } }

const tabs = [
  { id: 'general', label: 'Общие', icon: Settings },
  { id: 'notifications', label: 'Уведомления', icon: Bell },
  { id: 'security', label: 'Безопасность', icon: Shield },
  { id: 'data', label: 'Данные', icon: Database },
  { id: 'appearance', label: 'Внешний вид', icon: Palette },
  { id: 'hotkeys', label: 'Горячие клавиши', icon: Keyboard },
]

export function SettingsPage() {
  const [activeTab, setActiveTab] = useState('general')
  const [saved, setSaved] = useState(false)

  const handleSave = () => {
    setSaved(true)
    setTimeout(() => setSaved(false), 2000)
  }

  const renderTabContent = () => {
    switch (activeTab) {
      case 'general':
        return <GeneralSettings />
      case 'notifications':
        return <NotificationSettings />
      case 'security':
        return <SecuritySettings />
      case 'data':
        return <DataSettings />
      case 'appearance':
        return <AppearanceSettings />
      case 'hotkeys':
        return <HotkeySettings />
      default:
        return <GeneralSettings />
    }
  }

  return (
    <motion.div variants={container} initial="hidden" animate="show" className="p-6 h-full">
      {/* Header */}
      <motion.div variants={item} className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">Настройки</h1>
          <p className="text-sm text-text-secondary mt-1">Настройте приложение под себя</p>
        </div>
        <button 
          onClick={handleSave}
          className="btn btn-primary text-sm flex items-center gap-2"
        >
          {saved ? <Check size={16} /> : <Save size={16} />}
          {saved ? 'Сохранено' : 'Сохранить'}
        </button>
      </motion.div>

      <div className="flex gap-6 h-[calc(100%-80px)]">
        {/* Sidebar Tabs */}
        <motion.div variants={item} className="w-64 shrink-0 space-y-1">
          {tabs.map(tab => {
            const Icon = tab.icon
            const isActive = activeTab === tab.id
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left transition-all ${
                  isActive 
                    ? 'bg-accent/10 text-accent border border-accent/30' 
                    : 'text-text-secondary hover:bg-elevated/50 hover:text-text-primary'
                }`}
              >
                <Icon size={18} />
                <span className="text-sm font-medium">{tab.label}</span>
                {isActive && <ChevronRight size={14} className="ml-auto" />}
              </button>
            )
          })}
        </motion.div>

        {/* Tab Content */}
        <motion.div 
          variants={item} 
          className="flex-1 overflow-auto rounded-card border border-border bg-surface p-6"
        >
          <AnimatePresence mode="wait">
            <motion.div
              key={activeTab}
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -10 }}
              transition={{ duration: 0.2 }}
            >
              {renderTabContent()}
            </motion.div>
          </AnimatePresence>
        </motion.div>
      </div>
    </motion.div>
  )
}

// General Settings
function GeneralSettings() {
  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-text-primary">Общие настройки</h2>
      
      <div className="space-y-4">
        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">Автозапуск сканера</div>
            <div className="text-xs text-text-secondary">Запускать при старте приложения</div>
          </div>
          <input type="checkbox" defaultChecked className="w-4 h-4 rounded accent-accent" />
        </label>

        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">Свернуть в трей</div>
            <div className="text-xs text-text-secondary">При закрытии окна</div>
          </div>
          <input type="checkbox" defaultChecked className="w-4 h-4 rounded accent-accent" />
        </label>

        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">Звуковые уведомления</div>
            <div className="text-xs text-text-secondary">При найденной вилке</div>
          </div>
          <input type="checkbox" className="w-4 h-4 rounded accent-accent" />
        </label>

        <div className="p-4 rounded-lg bg-background">
          <div className="text-sm font-medium text-text-primary mb-2">Язык интерфейса</div>
          <select className="input w-full">
            <option>Русский</option>
            <option>English</option>
          </select>
        </div>
      </div>
    </div>
  )
}

// Notification Settings
function NotificationSettings() {
  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-text-primary">Уведомления</h2>
      
      <div className="space-y-3">
        {[
          { label: 'Найдена вилка', desc: 'Когда появляется новая арбитражная ситуация' },
          { label: 'Ставка размещена', desc: 'При успешном размещении ставки' },
          { label: 'Ошибка', desc: 'При любой ошибке в работе' },
          { label: 'Обновление баланса', desc: 'Когда меняется баланс в БК' },
          { label: 'Сессия истекает', desc: 'За 1 час до истечения сессии' },
        ].map((item, i) => (
          <label key={i} className="flex items-center justify-between p-4 rounded-lg bg-background">
            <div>
              <div className="text-sm font-medium text-text-primary">{item.label}</div>
              <div className="text-xs text-text-secondary">{item.desc}</div>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-xs text-text-muted">Push</span>
              <input type="checkbox" defaultChecked={i < 3} className="w-4 h-4 rounded accent-accent" />
              <span className="text-xs text-text-muted">TG</span>
              <input type="checkbox" className="w-4 h-4 rounded accent-accent" />
            </div>
          </label>
        ))}
      </div>
    </div>
  )
}

// Security Settings
function SecuritySettings() {
  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-text-primary">Безопасность</h2>
      
      <div className="p-4 rounded-lg bg-red-500/10 border border-red-500/20">
        <div className="flex items-start gap-3">
          <AlertTriangle size={20} className="text-red-400 shrink-0" />
          <div>
            <div className="text-sm font-medium text-red-300">Важно</div>
            <div className="text-xs text-text-secondary mt-1">
              Данные авторизации хранятся в зашифрованном виде. Никогда не передавайте файл сессии третьим лицам.
            </div>
          </div>
        </div>
      </div>

      <div className="space-y-4">
        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">Мастер-пароль</div>
            <div className="text-xs text-text-secondary">Защита данных авторизации</div>
          </div>
          <button className="btn btn-secondary text-xs">Изменить</button>
        </label>

        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">2FA для операций</div>
            <div className="text-xs text-text-secondary">Подтверждение ставок</div>
          </div>
          <input type="checkbox" className="w-4 h-4 rounded accent-accent" />
        </label>

        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">Автоблокировка</div>
            <div className="text-xs text-text-secondary">Через 5 минут бездействия</div>
          </div>
          <input type="checkbox" defaultChecked className="w-4 h-4 rounded accent-accent" />
        </label>
      </div>
    </div>
  )
}

// Data Settings
function DataSettings() {
  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-text-primary">Управление данными</h2>
      
      <div className="space-y-4">
        <div className="p-4 rounded-lg bg-background">
          <div className="flex items-center gap-3 mb-3">
            <Database size={20} className="text-accent" />
            <div>
              <div className="text-sm font-medium text-text-primary">База данных</div>
              <div className="text-xs text-text-secondary">Размер: 24.5 MB • 1,247 записей</div>
            </div>
          </div>
          <div className="flex gap-2">
            <button className="btn btn-secondary text-xs flex items-center gap-2">
              <Download size={14} /> Экспорт
            </button>
            <button className="btn btn-secondary text-xs flex items-center gap-2">
              <RotateCcw size={14} /> Оптимизировать
            </button>
          </div>
        </div>

        <div className="p-4 rounded-lg bg-background">
          <div className="flex items-center gap-3 mb-3">
            <FileText size={20} className="text-blue-400" />
            <div>
              <div className="text-sm font-medium text-text-primary">Логи</div>
              <div className="text-xs text-text-secondary">Последние 30 дней</div>
            </div>
          </div>
          <button className="btn btn-secondary text-xs flex items-center gap-2">
            <Download size={14} /> Скачать логи
          </button>
        </div>

        <div className="p-4 rounded-lg bg-red-500/10 border border-red-500/20">
          <div className="flex items-center gap-3 mb-3">
            <Trash2 size={20} className="text-red-400" />
            <div>
              <div className="text-sm font-medium text-text-primary">Опасная зона</div>
              <div className="text-xs text-text-secondary">Действия нельзя отменить</div>
            </div>
          </div>
          <div className="flex gap-2">
            <button className="btn btn-danger text-xs">Очистить историю</button>
            <button className="btn btn-danger text-xs">Сбросить все настройки</button>
          </div>
        </div>
      </div>
    </div>
  )
}

// Appearance Settings
function AppearanceSettings() {
  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-text-primary">Внешний вид</h2>
      
      <div className="space-y-4">
        <div className="p-4 rounded-lg bg-background">
          <div className="text-sm font-medium text-text-primary mb-3">Тема</div>
          <div className="flex gap-3">
            <button className="flex-1 p-3 rounded-lg border-2 border-accent bg-surface text-center">
              <Moon size={24} className="mx-auto mb-2 text-accent" />
              <div className="text-sm">Тёмная</div>
            </button>
            <button className="flex-1 p-3 rounded-lg border border-border bg-surface text-center opacity-50">
              <Sun size={24} className="mx-auto mb-2 text-text-muted" />
              <div className="text-sm">Светлая</div>
            </button>
          </div>
        </div>

        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">Анимации</div>
            <div className="text-xs text-text-secondary">Плавные переходы</div>
          </div>
          <input type="checkbox" defaultChecked className="w-4 h-4 rounded accent-accent" />
        </label>

        <label className="flex items-center justify-between p-4 rounded-lg bg-background">
          <div>
            <div className="text-sm font-medium text-text-primary">Компактный режим</div>
            <div className="text-xs text-text-secondary">Уменьшенные отступы</div>
          </div>
          <input type="checkbox" className="w-4 h-4 rounded accent-accent" />
        </label>
      </div>
    </div>
  )
}

// Hotkey Settings
function HotkeySettings() {
  const hotkeys = [
    { key: '⌘1', action: 'Обзор' },
    { key: '⌘2', action: 'Вилки' },
    { key: '⌘3', action: 'Коридоры' },
    { key: '⌘4', action: 'Экспрессы' },
    { key: '⌘5', action: 'Авто-ставки' },
    { key: '⌘6', action: 'Аккаунты' },
    { key: '⌘7', action: 'История' },
    { key: '⌘8', action: 'Настройки' },
    { key: '⌘B', action: 'Свернуть/развернуть sidebar' },
    { key: '⌘R', action: 'Обновить данные' },
    { key: '⌘F', action: 'Поиск' },
    { key: 'Esc', action: 'Закрыть модалку' },
  ]

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-semibold text-text-primary">Горячие клавиши</h2>
      
      <div className="grid grid-cols-2 gap-3">
        {hotkeys.map((hk, i) => (
          <div key={i} className="flex items-center justify-between p-3 rounded-lg bg-background">
            <span className="text-sm text-text-secondary">{hk.action}</span>
            <span className="text-xs px-2 py-1 rounded bg-surface border border-border text-text-primary font-mono">
              {hk.key}
            </span>
          </div>
        ))}
      </div>

      <div className="p-4 rounded-lg bg-background">
        <div className="text-sm font-medium text-text-primary mb-2">Свои горячие клавиши</div>
        <div className="text-xs text-text-secondary">В разработке</div>
      </div>
    </div>
  )
}
