import { useEffect, useMemo, useState } from 'react'
import { motion } from 'framer-motion'
import { Activity, ArrowUpDown, CalendarClock, ChevronRight, Filter, History, LayoutList, Search, Sparkles, Target, Wallet } from 'lucide-react'
import type { CorridorOpportunity, ExecutionLedgerAudit, ExpressFork, Surebet, ValueBet } from '../types'

interface HistoryPageProps {
  surebets: Surebet[]
  corridors: CorridorOpportunity[]
  expressForks: ExpressFork[]
  valueBets: ValueBet[]
  executionLedger: ExecutionLedgerAudit | null
}

type HistoryKind = 'all' | 'surebet' | 'corridor' | 'express' | 'value' | 'placement'
type GroupMode = 'day' | 'type' | 'bookmaker'

interface HistoryEntry {
  id: string
  timestamp: string
  kind: Exclude<HistoryKind, 'all'>
  title: string
  subtitle: string
  summary: string
  metric: string
  metricTone: 'positive' | 'negative' | 'neutral'
  badges: string[]
  bookmakerKeys: string[]
  searchText: string
  detailRows: Array<{ label: string, value: string }>
}

const FILTER_STORAGE_KEY = 'fork-history-filters'
const SELECTED_STORAGE_KEY = 'fork-history-selected-entry'

function formatDateTime(value: string | null) {
  if (!value) return '—'

  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return '—'

  return date.toLocaleString('ru-RU', {
    day: '2-digit',
    month: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function formatRelativeDay(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return 'Unknown'

  const now = new Date()
  const startOfNow = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const startOfDate = new Date(date.getFullYear(), date.getMonth(), date.getDate())
  const diffDays = Math.round((startOfNow.getTime() - startOfDate.getTime()) / 86400000)

  if (diffDays === 0) return 'Сегодня'
  if (diffDays === 1) return 'Вчера'

  return date.toLocaleDateString('ru-RU', {
    day: '2-digit',
    month: 'long',
  })
}

function formatPercent(value: number) {
  return `${value >= 0 ? '+' : ''}${value.toFixed(2)}%`
}

function formatMoney(value: number) {
  return `${value.toLocaleString('ru-RU', { maximumFractionDigits: 0 })} RUB`
}

function normalizeBookmaker(value: string) {
  return value.trim().toLowerCase()
}

function kindBadgeClass(kind: HistoryKind) {
  switch (kind) {
    case 'surebet':
      return 'badge-success'
    case 'corridor':
      return 'badge-info'
    case 'express':
      return 'badge-warning'
    case 'value':
      return 'badge-info'
    case 'placement':
      return 'badge-danger'
    default:
      return 'badge-info'
  }
}

function metricClass(tone: HistoryEntry['metricTone']) {
  if (tone === 'positive') return 'profit-positive'
  if (tone === 'negative') return 'profit-negative'
  return ''
}

function groupLabel(mode: GroupMode, entry: HistoryEntry) {
  if (mode === 'type') return entry.kind
  if (mode === 'bookmaker') return entry.bookmakerKeys[0] ?? 'unknown'
  return formatRelativeDay(entry.timestamp)
}

export function HistoryPage({ surebets, corridors, expressForks, valueBets, executionLedger }: HistoryPageProps) {
  const [search, setSearch] = useState('')
  const [kindFilter, setKindFilter] = useState<HistoryKind>('all')
  const [groupMode, setGroupMode] = useState<GroupMode>('day')
  const [bookmakerFilter, setBookmakerFilter] = useState('all')
  const [selectedEntryId, setSelectedEntryId] = useState<string | null>(null)

  useEffect(() => {
    try {
      const raw = window.localStorage.getItem(FILTER_STORAGE_KEY)
      if (!raw) return
      const parsed = JSON.parse(raw) as Partial<{ search: string, kindFilter: HistoryKind, groupMode: GroupMode, bookmakerFilter: string }>
      if (typeof parsed.search === 'string') setSearch(parsed.search)
      if (parsed.kindFilter) setKindFilter(parsed.kindFilter)
      if (parsed.groupMode) setGroupMode(parsed.groupMode)
      if (typeof parsed.bookmakerFilter === 'string') setBookmakerFilter(parsed.bookmakerFilter)
    } catch {
      // ignore invalid local storage payloads
    }

    const storedSelection = window.localStorage.getItem(SELECTED_STORAGE_KEY)
    if (storedSelection) setSelectedEntryId(storedSelection)
  }, [])

  useEffect(() => {
    window.localStorage.setItem(FILTER_STORAGE_KEY, JSON.stringify({ search, kindFilter, groupMode, bookmakerFilter }))
  }, [bookmakerFilter, groupMode, kindFilter, search])

  useEffect(() => {
    if (!selectedEntryId) {
      window.localStorage.removeItem(SELECTED_STORAGE_KEY)
      return
    }

    window.localStorage.setItem(SELECTED_STORAGE_KEY, selectedEntryId)
  }, [selectedEntryId])

  const entries = useMemo<HistoryEntry[]>(() => {
    const surebetEntries = surebets.map((entry) => ({
      id: `surebet-${entry.id}`,
      timestamp: entry.detected_at,
      kind: 'surebet' as const,
      title: `${entry.home_team} vs ${entry.away_team}`,
      subtitle: `${entry.sport} • ${entry.league}`,
      summary: `${entry.legs.length} legs · ${entry.legs.map((leg) => leg.bookmaker).join(' / ')}`,
      metric: formatPercent(entry.profit_percent),
      metricTone: 'positive' as const,
      badges: [entry.is_live ? 'live' : 'prematch', entry.verified ? 'verified' : 'new', entry.mirror ? 'mirror' : 'direct'],
      bookmakerKeys: [...new Set(entry.legs.map((leg) => normalizeBookmaker(leg.bookmaker)))],
      searchText: [entry.home_team, entry.away_team, entry.sport, entry.league, ...entry.legs.map((leg) => `${leg.bookmaker} ${leg.market} ${leg.selection}`)].join(' ').toLowerCase(),
      detailRows: [
        { label: 'Detected', value: formatDateTime(entry.detected_at) },
        { label: 'Profit', value: formatPercent(entry.profit_percent) },
        { label: 'Stake', value: formatMoney(entry.total_stake) },
        { label: 'Route', value: entry.legs.map((leg) => `${leg.bookmaker}:${leg.selection}@${leg.odds.toFixed(2)}`).join(' | ') },
      ],
    }))

    const corridorEntries = corridors.map((entry) => ({
      id: `corridor-${entry.id}`,
      timestamp: entry.detected_at,
      kind: 'corridor' as const,
      title: `${entry.home_team} vs ${entry.away_team}`,
      subtitle: `${entry.market} ${entry.line_low} / ${entry.line_high}`,
      summary: `${entry.sport} • ${entry.league}`,
      metric: formatPercent(entry.expected_roi),
      metricTone: entry.expected_roi > 0 ? 'positive' as const : 'neutral' as const,
      badges: [`double-win ${(entry.double_win_probability * 100).toFixed(0)}%`, 'read-only'],
      bookmakerKeys: [...new Set(entry.legs.map((leg) => normalizeBookmaker(leg.bookmaker)))],
      searchText: [entry.home_team, entry.away_team, entry.sport, entry.league, entry.market, ...entry.legs.map((leg) => `${leg.bookmaker} ${leg.selection}`)].join(' ').toLowerCase(),
      detailRows: [
        { label: 'Detected', value: formatDateTime(entry.detected_at) },
        { label: 'Expected ROI', value: formatPercent(entry.expected_roi) },
        { label: 'Double win', value: `${(entry.double_win_probability * 100).toFixed(1)}%` },
        { label: 'Books', value: entry.legs.map((leg) => `${leg.bookmaker}:${leg.selection}@${leg.odds.toFixed(2)}`).join(' | ') },
      ],
    }))

    const expressEntries = expressForks.map((entry) => ({
      id: `express-${entry.id}`,
      timestamp: entry.detected_at,
      kind: 'express' as const,
      title: `${entry.legs.length}-leg express hedge`,
      subtitle: `${entry.risk_level} risk • ${entry.verified ? 'verified' : 'watchlist'}`,
      summary: entry.legs.map((leg) => leg.bookmaker).join(' / '),
      metric: formatPercent(entry.profit_percent),
      metricTone: 'positive' as const,
      badges: ['express', entry.verified ? 'verified' : 'preview'],
      bookmakerKeys: [...new Set(entry.legs.map((leg) => normalizeBookmaker(leg.bookmaker)))],
      searchText: [entry.risk_level, ...entry.legs.flatMap((leg) => [leg.bookmaker, leg.selection, leg.market, leg.event.home_team, leg.event.away_team, ...leg.express_events])].join(' ').toLowerCase(),
      detailRows: [
        { label: 'Detected', value: formatDateTime(entry.detected_at) },
        { label: 'Profit', value: formatPercent(entry.profit_percent) },
        { label: 'Stake', value: formatMoney(entry.total_stake) },
        { label: 'Events', value: entry.legs.flatMap((leg) => leg.express_events.length > 0 ? leg.express_events : [`${leg.event.home_team} vs ${leg.event.away_team}`]).join(' | ') },
      ],
    }))

    const valueEntries = valueBets.map((entry) => ({
      id: `value-${entry.id}`,
      timestamp: entry.detected_at,
      kind: 'value' as const,
      title: `${entry.event.home_team} vs ${entry.event.away_team}`,
      subtitle: `${entry.market} • ${entry.selection}`,
      summary: `${entry.bookmaker} • fair ${entry.fair_odds.toFixed(2)} vs ${entry.odds.toFixed(2)}`,
      metric: formatPercent(entry.edge_percent),
      metricTone: entry.edge_percent > 0 ? 'positive' as const : 'neutral' as const,
      badges: [entry.event.is_live ? 'live' : 'prematch', entry.event.sport],
      bookmakerKeys: [normalizeBookmaker(entry.bookmaker)],
      searchText: [entry.bookmaker, entry.market, entry.selection, entry.event.home_team, entry.event.away_team, entry.event.league, entry.event.sport].join(' ').toLowerCase(),
      detailRows: [
        { label: 'Detected', value: formatDateTime(entry.detected_at) },
        { label: 'Edge', value: formatPercent(entry.edge_percent) },
        { label: 'Fair odds', value: entry.fair_odds.toFixed(2) },
        { label: 'Posted odds', value: entry.odds.toFixed(2) },
      ],
    }))

    const placementEntries = (executionLedger?.recent_records ?? []).map((record) => ({
      id: `placement-${record.placement.id}-${record.recorded_at}-${record.action}`,
      timestamp: record.recorded_at,
      kind: 'placement' as const,
      title: `${record.placement.event.home_team} vs ${record.placement.event.away_team}`,
      subtitle: `${record.placement.bookmaker} • ${record.action}`,
      summary: `${record.placement.market} / ${record.placement.selection}`,
      metric: formatMoney(record.placement.stake),
      metricTone: record.placement.error ? 'negative' as const : 'neutral' as const,
      badges: [record.placement.status, record.placement.error ? 'error' : 'ok'],
      bookmakerKeys: [normalizeBookmaker(record.placement.bookmaker)],
      searchText: [record.placement.bookmaker, record.action, record.placement.market, record.placement.selection, record.placement.event.home_team, record.placement.event.away_team, record.placement.event.league].join(' ').toLowerCase(),
      detailRows: [
        { label: 'Recorded', value: formatDateTime(record.recorded_at) },
        { label: 'Stake / odds', value: `${formatMoney(record.placement.stake)} @ ${record.placement.odds.toFixed(2)}` },
        { label: 'Status', value: record.placement.status },
        { label: 'Error', value: record.placement.error ?? '—' },
      ],
    }))

    return [...placementEntries, ...surebetEntries, ...corridorEntries, ...expressEntries, ...valueEntries]
      .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
  }, [corridors, executionLedger, expressForks, surebets, valueBets])

  const availableBookmakers = useMemo(() => {
    const keys = new Set<string>()
    entries.forEach((entry) => entry.bookmakerKeys.forEach((bookmaker) => keys.add(bookmaker)))
    return ['all', ...Array.from(keys).sort()]
  }, [entries])

  const filteredEntries = useMemo(() => {
    const normalizedSearch = search.trim().toLowerCase()

    return entries.filter((entry) => {
      if (kindFilter !== 'all' && entry.kind !== kindFilter) return false
      if (bookmakerFilter !== 'all' && !entry.bookmakerKeys.includes(bookmakerFilter)) return false
      if (normalizedSearch && !entry.searchText.includes(normalizedSearch)) return false
      return true
    })
  }, [bookmakerFilter, entries, kindFilter, search])

  const groupedEntries = useMemo(() => {
    const groups = new Map<string, HistoryEntry[]>()
    filteredEntries.forEach((entry) => {
      const label = groupLabel(groupMode, entry)
      const bucket = groups.get(label) ?? []
      bucket.push(entry)
      groups.set(label, bucket)
    })

    return Array.from(groups.entries())
  }, [filteredEntries, groupMode])

  useEffect(() => {
    if (!filteredEntries.length) {
      setSelectedEntryId(null)
      return
    }

    if (!selectedEntryId || !filteredEntries.some((entry) => entry.id === selectedEntryId)) {
      setSelectedEntryId(filteredEntries[0].id)
    }
  }, [filteredEntries, selectedEntryId])

  const selectedEntry = filteredEntries.find((entry) => entry.id === selectedEntryId) ?? null

  const totals = useMemo(() => ({
    surebets: entries.filter((entry) => entry.kind === 'surebet').length,
    placements: entries.filter((entry) => entry.kind === 'placement').length,
    watchlist: entries.filter((entry) => entry.kind === 'corridor' || entry.kind === 'express' || entry.kind === 'value').length,
  }), [entries])

  return (
    <motion.div
      className="space-y-6"
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
        <div>
          <h2 className="text-2xl font-bold">Activity / History</h2>
          <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>
            Read-only timeline over surebets, corridors, express, value bets and execution ledger snapshots.
          </p>
        </div>

        <div className="flex flex-wrap gap-3 text-xs">
          <div className="rounded-xl px-3 py-2" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            {filteredEntries.length} visible entries
          </div>
          <div className="rounded-xl px-3 py-2" style={{ background: 'var(--bg-card)', border: '1px solid var(--border-color)' }}>
            Selection persists locally
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-4 gap-4">
        <div className="glass-card p-5">
          <div className="flex items-start justify-between mb-3">
            <div>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Merged feed</p>
              <p className="text-xl font-semibold mt-1">{entries.length}</p>
            </div>
            <History size={18} style={{ color: 'var(--accent-blue)' }} />
          </div>
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>Unified snapshot across read-only endpoints already used in UI.</p>
        </div>

        <div className="glass-card p-5">
          <div className="flex items-start justify-between mb-3">
            <div>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Surebet feed</p>
              <p className="text-xl font-semibold mt-1">{totals.surebets}</p>
            </div>
            <Sparkles size={18} style={{ color: 'var(--accent-green)' }} />
          </div>
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>Fast scan for recent edges without opening each market surface.</p>
        </div>

        <div className="glass-card p-5">
          <div className="flex items-start justify-between mb-3">
            <div>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Execution records</p>
              <p className="text-xl font-semibold mt-1">{totals.placements}</p>
            </div>
            <Wallet size={18} style={{ color: 'var(--accent-red)' }} />
          </div>
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>Placement actions stay in the same timeline for quick operational replay.</p>
        </div>

        <div className="glass-card p-5">
          <div className="flex items-start justify-between mb-3">
            <div>
              <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Watchlist items</p>
              <p className="text-xl font-semibold mt-1">{totals.watchlist}</p>
            </div>
            <Target size={18} style={{ color: 'var(--accent-yellow)' }} />
          </div>
          <p className="text-sm" style={{ color: 'var(--text-secondary)' }}>Corridors, express and value ideas remain searchable even when no bets were placed.</p>
        </div>
      </div>

      <div className="glass-card p-5 space-y-4">
        <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
          <div>
            <h3 className="text-base font-semibold">Client-side filters</h3>
            <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Search, type slicing, bookmaker focus and grouping are persisted in local storage.</p>
          </div>

          <div className="flex items-center gap-2 text-xs" style={{ color: 'var(--text-secondary)' }}>
            <Activity size={14} />
            read-only UX layer
          </div>
        </div>

        <div className="grid grid-cols-1 xl:grid-cols-[1.4fr,0.8fr,0.8fr,0.9fr] gap-3">
          <label className="relative block">
            <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2" style={{ color: 'var(--text-muted)' }} />
            <input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search team, market, bookmaker, action" className="input pl-9" />
          </label>

          <label className="block">
            <span className="text-xs mb-1.5 inline-flex items-center gap-2" style={{ color: 'var(--text-muted)' }}><Filter size={13} /> Type</span>
            <select value={kindFilter} onChange={(event) => setKindFilter(event.target.value as HistoryKind)} className="input">
              <option value="all">All activity</option>
              <option value="surebet">Surebets</option>
              <option value="corridor">Corridors</option>
              <option value="express">Express</option>
              <option value="value">Value bets</option>
              <option value="placement">Placements</option>
            </select>
          </label>

          <label className="block">
            <span className="text-xs mb-1.5 inline-flex items-center gap-2" style={{ color: 'var(--text-muted)' }}><ArrowUpDown size={13} /> Group</span>
            <select value={groupMode} onChange={(event) => setGroupMode(event.target.value as GroupMode)} className="input">
              <option value="day">By day</option>
              <option value="type">By type</option>
              <option value="bookmaker">By bookmaker</option>
            </select>
          </label>

          <label className="block">
            <span className="text-xs mb-1.5 inline-flex items-center gap-2" style={{ color: 'var(--text-muted)' }}><LayoutList size={13} /> Bookmaker</span>
            <select value={bookmakerFilter} onChange={(event) => setBookmakerFilter(event.target.value)} className="input">
              {availableBookmakers.map((bookmaker) => (
                <option key={bookmaker} value={bookmaker}>{bookmaker === 'all' ? 'All books' : bookmaker}</option>
              ))}
            </select>
          </label>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[1.3fr,0.7fr] gap-6">
        <div className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Timeline groups</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Every row opens persisted detail context on the right.</p>
            </div>
            <span className="badge badge-info">{groupedEntries.length} groups</span>
          </div>

          <div className="space-y-5">
            {groupedEntries.length > 0 ? groupedEntries.map(([label, group]) => (
              <div key={label} className="space-y-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-2">
                    <CalendarClock size={15} style={{ color: 'var(--accent-cyan)' }} />
                    <h4 className="text-sm font-semibold capitalize">{label}</h4>
                  </div>
                  <span className="badge badge-info">{group.length}</span>
                </div>

                <div className="space-y-3">
                  {group.map((entry) => (
                    <button
                      key={entry.id}
                      type="button"
                      onClick={() => setSelectedEntryId(entry.id)}
                      className="w-full rounded-xl p-4 text-left transition-colors"
                      style={{
                        background: selectedEntryId === entry.id ? 'var(--bg-hover)' : 'var(--bg-secondary)',
                        border: `1px solid ${selectedEntryId === entry.id ? 'rgba(88, 166, 255, 0.28)' : 'var(--border-color)'}`,
                      }}
                    >
                      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-center gap-2 mb-2">
                            <span className={`badge ${kindBadgeClass(entry.kind)}`}>{entry.kind}</span>
                            {entry.badges.slice(0, 3).map((badge) => <span key={`${entry.id}-${badge}`} className="badge badge-info">{badge}</span>)}
                          </div>
                          <p className="text-sm font-semibold truncate">{entry.title}</p>
                          <p className="text-xs mt-1" style={{ color: 'var(--text-secondary)' }}>{entry.subtitle}</p>
                          <p className="text-xs mt-2 leading-5" style={{ color: 'var(--text-muted)' }}>{entry.summary}</p>
                        </div>

                        <div className="flex items-center justify-between gap-4 lg:flex-col lg:items-end">
                          <div className="text-right">
                            <p className={`text-sm font-semibold ${metricClass(entry.metricTone)}`}>{entry.metric}</p>
                            <p className="text-xs mt-1" style={{ color: 'var(--text-muted)' }}>{formatDateTime(entry.timestamp)}</p>
                          </div>
                          <ChevronRight size={16} style={{ color: 'var(--text-muted)' }} />
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              </div>
            )) : (
              <div className="rounded-xl border border-dashed p-8 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
                <History size={24} className="mx-auto mb-3 opacity-40" />
                <p className="text-sm">Текущие client-side фильтры скрыли все записи истории.</p>
              </div>
            )}
          </div>
        </div>

        <div className="glass-card p-5">
          <div className="flex items-center justify-between mb-4 gap-4">
            <div>
              <h3 className="text-base font-semibold">Details pane</h3>
              <p className="text-xs mt-0.5" style={{ color: 'var(--text-muted)' }}>Last selected row is restored after reload.</p>
            </div>
            {selectedEntry ? <span className={`badge ${kindBadgeClass(selectedEntry.kind)}`}>{selectedEntry.kind}</span> : null}
          </div>

          {selectedEntry ? (
            <div className="space-y-4">
              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <p className="text-lg font-semibold leading-7">{selectedEntry.title}</p>
                <p className="text-sm mt-1" style={{ color: 'var(--text-secondary)' }}>{selectedEntry.subtitle}</p>
                <div className="flex flex-wrap gap-2 mt-3">
                  {selectedEntry.badges.map((badge) => <span key={`detail-${selectedEntry.id}-${badge}`} className="badge badge-info">{badge}</span>)}
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                  <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Primary metric</p>
                  <p className={`text-lg font-semibold mt-2 ${metricClass(selectedEntry.metricTone)}`}>{selectedEntry.metric}</p>
                </div>
                <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                  <p className="text-xs uppercase tracking-wider" style={{ color: 'var(--text-muted)' }}>Bookmakers</p>
                  <p className="text-sm font-semibold mt-2 break-words">{selectedEntry.bookmakerKeys.join(', ') || '—'}</p>
                </div>
              </div>

              <div className="rounded-xl p-4" style={{ background: 'var(--bg-secondary)', border: '1px solid var(--border-color)' }}>
                <p className="text-xs uppercase tracking-wider mb-3" style={{ color: 'var(--text-muted)' }}>Event detail</p>
                <div className="space-y-3">
                  {selectedEntry.detailRows.map((row) => (
                    <div key={`${selectedEntry.id}-${row.label}`} className="flex items-start justify-between gap-4 text-sm">
                      <span style={{ color: 'var(--text-secondary)' }}>{row.label}</span>
                      <span className="text-right break-words max-w-[60%]">{row.value}</span>
                    </div>
                  ))}
                </div>
              </div>

              <div className="rounded-xl p-4" style={{ background: 'rgba(88, 166, 255, 0.08)', border: '1px solid rgba(88, 166, 255, 0.2)' }}>
                <p className="text-xs uppercase tracking-wider mb-2" style={{ color: 'var(--text-muted)' }}>Why this page matters</p>
                <p className="text-sm leading-6" style={{ color: 'var(--text-secondary)' }}>
                  Operator can replay what surfaced, which bookmaker path was involved, and how execution records align with discovery events without leaving the desktop UI.
                </p>
              </div>
            </div>
          ) : (
            <div className="rounded-xl border border-dashed p-8 text-center" style={{ borderColor: 'var(--border-color)', color: 'var(--text-muted)' }}>
              <History size={24} className="mx-auto mb-3 opacity-40" />
              <p className="text-sm">Нет выбранной записи.</p>
            </div>
          )}
        </div>
      </div>
    </motion.div>
  )
}
