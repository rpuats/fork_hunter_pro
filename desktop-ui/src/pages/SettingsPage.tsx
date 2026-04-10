import { motion } from 'framer-motion'
import { Settings, Save, Bell, Shield, Database, Zap } from 'lucide-react'
import { toast } from 'sonner'

export function SettingsPage() {
  const handleSave = () => {
    toast.success('Настройки сохранены')
  }

  return (
    <motion.div 
      className="space-y-6"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Настройки</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Конфигурация сканера, букмекеров и уведомлений
          </p>
        </div>
        <button onClick={handleSave} className="btn btn-primary">
          <Save size={16} />
          Сохранить
        </button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Scanner Settings */}
        <motion.div className="glass-card p-5" variants={{ hidden: { opacity: 0, y: 20 }, show: { opacity: 1, y: 0 } }}>
          <div className="flex items-center gap-3 mb-5">
            <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'linear-gradient(135deg, #58a6ff 0%, #bc8cff 100%)' }}>
              <Zap size={20} color="#fff" />
            </div>
            <div>
              <h3 className="text-base font-semibold">Сканер</h3>
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Основные параметры поиска</p>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium mb-1.5">Минимальная прибыль (%)</label>
              <input type="number" defaultValue={1} step={0.1} className="input" />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1.5">Максимальная прибыль (%)</label>
              <input type="number" defaultValue={30} step={1} className="input" />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1.5">Размер банкролла (₽)</label>
              <input type="number" defaultValue={100000} step={10000} className="input" />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1.5">Интервал сканирования (сек)</label>
              <input type="number" defaultValue={15} step={5} className="input" />
            </div>
          </div>
        </motion.div>

        {/* Bookmakers */}
        <motion.div className="glass-card p-5">
          <div className="flex items-center gap-3 mb-5">
            <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'linear-gradient(135deg, #3fb950 0%, #39d2c0 100%)' }}>
              <Database size={20} color="#fff" />
            </div>
            <div>
              <h3 className="text-base font-semibold">Букмекеры</h3>
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Активные источники данных</p>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            {[
              { name: 'Pari', events: '6,608' },
              { name: 'Fonbet', events: '6,826' },
              { name: 'Bettery', events: '6,843' },
              { name: 'Marathon', events: '6,566' },
              { name: '24bet', events: '6,557' },
              { name: 'Leon', events: '3,676' },
              { name: 'Sportbet', events: '258' },
            ].map(bk => (
              <label key={bk.name} className="flex items-center gap-3 p-3 rounded-lg cursor-pointer transition-colors" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <input type="checkbox" defaultChecked className="w-4 h-4 rounded" style={{ accentColor: 'var(--accent-blue)' }} />
                <div className="flex-1">
                  <p className="text-sm font-medium capitalize">{bk.name}</p>
                  <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{bk.events} событий</p>
                </div>
              </label>
            ))}
          </div>
        </motion.div>

        {/* Notifications */}
        <motion.div className="glass-card p-5">
          <div className="flex items-center gap-3 mb-5">
            <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'linear-gradient(135deg, #d29922 0%, #f0883e 100%)' }}>
              <Bell size={20} color="#fff" />
            </div>
            <div>
              <h3 className="text-base font-semibold">Уведомления</h3>
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Оповещения о новых вилках</p>
            </div>
          </div>

          <div className="space-y-3">
            {[
              { label: 'Telegram уведомления', desc: 'Отправлять вилки в Telegram бота' },
              { label: 'Звуковые уведомления', desc: 'Звук при нахождении вилки > 3%' },
              { label: 'Push уведомления', desc: 'Браузерные push уведомления' },
              { label: 'Email рассылка', desc: 'Ежедневная сводка на email' },
            ].map((item, i) => (
              <label key={i} className="flex items-start gap-3 p-3 rounded-lg cursor-pointer transition-colors" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <input type="checkbox" defaultChecked={i < 2} className="w-4 h-4 rounded mt-0.5" style={{ accentColor: 'var(--accent-blue)' }} />
                <div>
                  <p className="text-sm font-medium">{item.label}</p>
                  <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>{item.desc}</p>
                </div>
              </label>
            ))}
          </div>
        </motion.div>

        {/* Advanced */}
        <motion.div className="glass-card p-5">
          <div className="flex items-center gap-3 mb-5">
            <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'linear-gradient(135deg, #f85149 0%, #f0883e 100%)' }}>
              <Shield size={20} color="#fff" />
            </div>
            <div>
              <h3 className="text-base font-semibold">Дополнительно</h3>
              <p className="text-xs" style={{ color: 'var(--text-muted)' }}>Расширенные настройки</p>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <label className="block text-sm font-medium mb-1.5">Telegram Bot Token</label>
              <input type="password" placeholder="123456:ABC-DEF..." className="input" />
            </div>
            <div>
              <label className="block text-sm font-medium mb-1.5">Telegram Chat ID</label>
              <input type="text" placeholder="-1001234567890" className="input" />
            </div>
            <div className="flex items-center justify-between p-3 rounded-lg" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
              <div>
                <p className="text-sm font-medium">Автоставки</p>
                <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Автоматическое размещение ставок</p>
              </div>
              <label className="relative inline-flex items-center cursor-pointer">
                <input type="checkbox" className="sr-only peer" />
                <div className="w-11 h-6 bg-gray-700 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-[var(--accent-blue)]"></div>
              </label>
            </div>
          </div>
        </motion.div>
      </div>
    </motion.div>
  )
}
