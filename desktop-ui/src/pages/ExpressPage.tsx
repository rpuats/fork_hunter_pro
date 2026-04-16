import { useMemo, useState } from 'react'
import { motion } from 'framer-motion'
import { Layers, ShieldCheck, AlertTriangle, CheckCircle2, Info, ArrowUpDown, Copy, Radio } from 'lucide-react'
import { toast } from 'sonner'
import type { ExpressFork, ExpressForkLeg, ExpressRiskLevel } from '../types'

interface ExpressPageProps {
  expressForks: ExpressFork[]
}

const riskConfig: Record<ExpressRiskLevel, { label: string, color: string, icon: typeof CheckCircle2 }> = {
  low: { label: 'Низкий', color: 'var(--accent-green)', icon: CheckCircle2 },
  medium: { label: 'Средний', color: 'var(--accent-yellow)', icon: Info },
  high: { label: 'Высокий', color: 'var(--accent-red)', icon: AlertTriangle },
}

function normalizeRiskLevel(value: ExpressFork['risk_level']): ExpressRiskLevel {
  switch (value) {
    case 'Low':
      return 'low'
    case 'Medium':
      return 'medium'
    case 'High':
      return 'high'
    default:
      return value
  }
}

function formatMoney(value: number) {
  return `${Math.round(value).toLocaleString('ru-RU')}₽`
}

function formatEventLabel(leg: ExpressForkLeg) {
  if (leg.is_express) {
    return leg.express_events.length ? `${leg.express_events.length} событий в экспрессе` : 'Собранный экспресс'
  }

  return `${leg.event.home_team} - ${leg.event.away_team}`
}

function formatDetectedAt(value: string) {
  return new Date(value).toLocaleString('ru-RU', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function buildClipboardText(expressFork: ExpressFork) {
  return expressFork.legs.map((leg, index) => {
    const title = formatEventLabel(leg)
    return `${index + 1}. ${title}\n${leg.bookmaker}: ${leg.market} / ${leg.selection} @ ${leg.odds.toFixed(2)}\nСумма: ${formatMoney(leg.stake ?? 0)}`
  }).join('\n\n')
}

export function ExpressPage({ expressForks }: ExpressPageProps) {
  const [riskFilter, setRiskFilter] = useState<'all' | ExpressRiskLevel>('all')
  const [verificationFilter, setVerificationFilter] = useState<'all' | 'verified' | 'unverified'>('all')
  const [sortBy, setSortBy] = useState<'profit' | 'time'>('profit')

  const filtered = useMemo(() => {
    const result = expressForks.filter((fork) => {
      if (riskFilter !== 'all' && normalizeRiskLevel(fork.risk_level) !== riskFilter) return false
      if (verificationFilter === 'verified' && !fork.verified) return false
      if (verificationFilter === 'unverified' && fork.verified) return false
      return true
    })

    result.sort((a, b) => {
      if (sortBy === 'profit') return b.profit_percent - a.profit_percent
      return new Date(b.detected_at).getTime() - new Date(a.detected_at).getTime()
    })

    return result
  }, [expressForks, riskFilter, sortBy, verificationFilter])

  const verifiedCount = expressForks.filter((fork) => fork.verified).length
  const liveCoverage = expressForks.filter((fork) => fork.legs.some((leg) => leg.event.is_live)).length

  return (
    <motion.div
      className="space-y-6"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
        <div>
          <h2 className="text-2xl font-bold">Экспресс-вилки</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Живой срез по `/api/v1/express-forks` без экранных заглушек и demo-сценариев
          </p>
        </div>

        <div className="grid gap-3 sm:grid-cols-3">
          <div className="glass-card px-4 py-3 min-w-[160px]">
            <p className="text-xs mb-1" style={{ color: 'var(--text-muted)' }}>В кэше</p>
            <p className="text-xl font-semibold">{expressForks.length}</p>
          </div>
          <div className="glass-card px-4 py-3 min-w-[160px]">
            <p className="text-xs mb-1" style={{ color: 'var(--text-muted)' }}>Верифицировано</p>
            <p className="text-xl font-semibold">{verifiedCount}</p>
          </div>
          <div className="glass-card px-4 py-3 min-w-[160px]">
            <p className="text-xs mb-1" style={{ color: 'var(--text-muted)' }}>Live покрытие</p>
            <p className="text-xl font-semibold">{liveCoverage}</p>
          </div>
        </div>
      </div>

      <motion.div
        className="glass-card p-4"
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
      >
        <div className="flex flex-wrap gap-3 items-center">
          <select value={riskFilter} onChange={(e) => setRiskFilter(e.target.value as 'all' | ExpressRiskLevel)} className="input !w-auto !py-2">
            <option value="all">Все риски</option>
            <option value="low">Низкий риск</option>
            <option value="medium">Средний риск</option>
            <option value="high">Высокий риск</option>
          </select>

          <select value={verificationFilter} onChange={(e) => setVerificationFilter(e.target.value as 'all' | 'verified' | 'unverified')} className="input !w-auto !py-2">
            <option value="all">Любая проверка</option>
            <option value="verified">Только verified</option>
            <option value="unverified">Только draft</option>
          </select>

          <button onClick={() => setSortBy(sortBy === 'profit' ? 'time' : 'profit')} className="btn btn-ghost">
            <ArrowUpDown size={14} />
            {sortBy === 'profit' ? 'По прибыли' : 'По времени'}
          </button>
        </div>
      </motion.div>

      {filtered.length > 0 ? (
        <div className="space-y-4">
          {filtered.map((fork, index) => {
            const riskLevel = normalizeRiskLevel(fork.risk_level)
            const RiskIcon = riskConfig[riskLevel].icon
            const expressLeg = fork.legs.find((leg) => leg.is_express)
            const hedgeLegs = fork.legs.filter((leg) => !leg.is_express)

            return (
              <motion.div
                key={fork.id}
                className="glass-card p-5"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: index * 0.04 }}
              >
                <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between mb-4">
                  <div className="flex items-start gap-3">
                    <div className="w-10 h-10 rounded-xl flex items-center justify-center" style={{ background: 'var(--gradient-primary)' }}>
                      <Layers size={20} color="#fff" />
                    </div>

                    <div>
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="text-base font-semibold">Express #{index + 1}</h3>
                        <span className="badge badge-info">{fork.legs.length} legs</span>
                        {fork.verified ? (
                          <span className="badge" style={{ background: 'rgba(34,197,94,0.15)', color: 'var(--accent-green)' }}>
                            <ShieldCheck size={12} /> verified
                          </span>
                        ) : (
                          <span className="badge" style={{ background: 'rgba(245,158,11,0.16)', color: 'var(--accent-yellow)' }}>
                            draft
                          </span>
                        )}
                        {fork.legs.some((leg) => leg.event.is_live) && (
                          <span className="badge" style={{ background: 'rgba(6,182,212,0.16)', color: 'var(--accent-cyan)' }}>
                            <Radio size={12} /> live
                          </span>
                        )}
                      </div>

                      <div className="flex items-center gap-2 mt-2">
                        <RiskIcon size={14} style={{ color: riskConfig[riskLevel].color }} />
                        <span className="text-xs" style={{ color: riskConfig[riskLevel].color }}>
                          Риск: {riskConfig[riskLevel].label}
                        </span>
                        <span className="text-xs" style={{ color: 'var(--text-muted)' }}>
                          Обновлено {formatDetectedAt(fork.detected_at)}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="text-right">
                    <div className="profit profit-positive text-2xl">+{fork.profit_percent.toFixed(2)}%</div>
                    <p className="text-sm mt-1" style={{ color: 'var(--text-muted)' }}>
                      Общий объём: {formatMoney(fork.total_stake)}
                    </p>
                  </div>
                </div>

                <div className="grid gap-4 lg:grid-cols-[1.1fr_0.9fr]">
                  <div className="space-y-3">
                    {expressLeg && (
                      <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                        <div className="flex items-center justify-between gap-3 mb-2">
                          <div>
                            <p className="text-sm font-semibold">Собранный экспресс</p>
                            <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                              {expressLeg.express_events.length} событий в плече `{expressLeg.selection}`
                            </p>
                          </div>
                          <div className="text-right">
                            <p className="text-sm font-mono">@ {expressLeg.odds.toFixed(2)}</p>
                            <p className="text-xs" style={{ color: 'var(--text-muted)' }}>{formatMoney(expressLeg.stake ?? 0)}</p>
                          </div>
                        </div>

                        <div className="space-y-2">
                          {expressLeg.express_events.map((eventName) => (
                            <div key={eventName} className="text-sm rounded-lg px-3 py-2" style={{ background: 'rgba(255,255,255,0.02)' }}>
                              {eventName}
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {hedgeLegs.map((leg) => (
                      <div key={`${fork.id}-${leg.bookmaker}-${leg.event.id}`} className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                        <div className="flex items-start justify-between gap-3">
                          <div>
                            <p className="text-sm font-semibold">{formatEventLabel(leg)}</p>
                            <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>
                              {leg.event.league || leg.event.sport} • {leg.market} / {leg.selection}
                            </p>
                          </div>

                          <div className="text-right">
                            <p className="text-sm font-medium capitalize">{leg.bookmaker}</p>
                            <p className="text-sm font-mono">@ {leg.odds.toFixed(2)}</p>
                          </div>
                        </div>

                        <div className="flex flex-wrap items-center justify-between gap-2 mt-3 text-xs" style={{ color: 'var(--text-muted)' }}>
                          <span>{leg.event.is_live ? 'Live' : 'Prematch'}</span>
                          <span>{formatMoney(leg.stake ?? 0)}</span>
                        </div>
                      </div>
                    ))}
                  </div>

                  <div className="rounded-xl p-4 h-fit" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                    <p className="text-sm font-semibold mb-3">Контракт ответа</p>

                    <div className="space-y-3 text-sm">
                      <div className="flex items-center justify-between gap-3">
                        <span style={{ color: 'var(--text-muted)' }}>ID</span>
                        <span className="font-mono text-xs">{fork.id.slice(0, 8)}</span>
                      </div>
                      <div className="flex items-center justify-between gap-3">
                        <span style={{ color: 'var(--text-muted)' }}>Verified</span>
                        <span>{fork.verified ? 'true' : 'false'}</span>
                      </div>
                      <div className="flex items-center justify-between gap-3">
                        <span style={{ color: 'var(--text-muted)' }}>Hedge legs</span>
                        <span>{hedgeLegs.length}</span>
                      </div>
                      <div className="flex items-center justify-between gap-3">
                        <span style={{ color: 'var(--text-muted)' }}>Express events</span>
                        <span>{expressLeg?.express_events.length ?? 0}</span>
                      </div>
                    </div>

                    <button
                      onClick={() => {
                        navigator.clipboard.writeText(buildClipboardText(fork))
                        toast.success('План ставок скопирован')
                      }}
                      className="btn btn-primary w-full justify-center mt-4"
                    >
                      <Copy size={16} />
                      Копировать план
                    </button>
                  </div>
                </div>
              </motion.div>
            )
          })}
        </div>
      ) : (
        <div className="glass-card p-16 text-center">
          <Layers size={56} className="mx-auto mb-4 opacity-30" style={{ color: 'var(--accent-cyan)' }} />
          <h3 className="text-xl font-bold mb-2">Экспресс-вилки не пришли с backend</h3>
          <p className="text-sm mb-2" style={{ color: 'var(--text-muted)' }}>
            Экран ждёт ответ от `/api/v1/express-forks` и не подменяет состояние mock-данными.
          </p>
        </div>
      )}
    </motion.div>
  )
}
