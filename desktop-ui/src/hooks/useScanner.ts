import { useState, useEffect, useCallback, useRef } from 'react'
import { toast } from 'sonner'
import type {
  AccountSessionSummary,
  AccountStateResponse,
  ApiResponse,
  BackendBookmaker,
  BackendCollectionResponse,
  BackendCorridorOpportunity,
  BankrollRecommendationsResponse,
  BankrollState,
  ExecutionLedgerAudit,
  ExecutionStateAudit,
  BackendGenerosityIndex,
  ExecutionOverview,
  ParserCoverage,
  ParserHealth,
  BackendSurebet,
  Bookmaker,
  CorridorOpportunity,
  ExpressFork,
  GenerosityIndex,
  ScannerMetrics,
  ScannerStatus,
  Surebet,
  ValueBet,
} from '../types'

const WS_URL = 'ws://localhost:8080/ws/v1/surebets'
const API_BASE = 'http://localhost:8080'

type LegacyBookmakersResponse = {
  bookmakers?: BackendBookmaker[]
}

type LegacyWsMessage = {
  type?: string
  data?: unknown
}

type CompatWsMessage = LegacyWsMessage & {
  event?: string
  channel?: string
  version?: string
}

type RawSurebetBusMessage = {
  SurebetFound?: {
    surebet_id?: string
    payload?: unknown
    timestamp?: string
  }
}

function unwrapCollection<T>(payload: T[] | BackendCollectionResponse<T> | null, key: 'surebets' | 'corridors'): T[] {
  if (Array.isArray(payload)) return payload
  if (payload && Array.isArray(payload[key])) return payload[key]
  return []
}

function mapSurebet(surebet: BackendSurebet): Surebet {
  return {
    ...surebet,
    id: String(surebet.id),
    sport: String(surebet.sport),
    profitPercent: surebet.profit_percent,
  }
}

function mapBookmaker(bookmaker: BackendBookmaker): Bookmaker {
  const enabled = bookmaker.enabled ?? true
  const backendStatus = bookmaker.status
    ?? (!enabled
      ? 'disabled'
      : bookmaker.execution_supported
        ? 'execution_ready'
        : 'scan_only')
  const status = enabled
    ? 'active'
    : backendStatus === 'disabled'
      ? 'inactive'
      : 'error'

  return {
    slug: bookmaker.slug ?? bookmaker.id ?? bookmaker.name.toLowerCase(),
    name: bookmaker.name,
    status,
    events: 0,
    odds: 0,
    last_update: null,
    enabled,
    scan_supported: bookmaker.scan_supported ?? enabled,
    execution_supported: bookmaker.execution_supported ?? false,
    backend_status: backendStatus,
    notes: bookmaker.notes ?? null,
  }
}

function normalizeBookmakersPayload(payload: BackendBookmaker[] | LegacyBookmakersResponse | null): Bookmaker[] {
  if (Array.isArray(payload)) return payload.map(mapBookmaker)
  if (payload && Array.isArray(payload.bookmakers)) return payload.bookmakers.map(mapBookmaker)
  return []
}

function normalizeGenerosityPayload(
  payload: BackendGenerosityIndex[] | { ranking?: BackendGenerosityIndex[] } | null,
): GenerosityIndex[] {
  if (Array.isArray(payload)) return payload
  if (payload && Array.isArray(payload.ranking)) return payload.ranking
  return []
}

function normalizeMetricsPayload(payload: unknown): ScannerMetrics | null {
  if (!payload || typeof payload !== 'object') return null

  const data = payload as Partial<ScannerMetrics> & {
    last_cycle_time_ms?: number
    total_events?: number
    total_surebets?: number
    parsers?: Record<string, { events?: number, error?: string }>
    cache_stats?: { hit_rate?: number }
    performance?: { memory?: { current_mb?: number } }
  }

  if (typeof data.cycle_time_ms === 'number') {
    return {
      cycle_time_ms: data.cycle_time_ms,
      events_parsed: data.events_parsed ?? 0,
      surebets_found: data.surebets_found ?? 0,
      active_bookmakers: data.active_bookmakers ?? 0,
      failed_bookmakers: data.failed_bookmakers ?? 0,
      cache_hit_rate: data.cache_hit_rate ?? 0,
      memory_mb: data.memory_mb ?? 0,
      timestamp: data.timestamp ?? new Date().toISOString(),
    }
  }

  const parsers = data.parsers ? Object.values(data.parsers) : []
  return {
    cycle_time_ms: data.last_cycle_time_ms ?? 0,
    events_parsed: data.total_events ?? 0,
    surebets_found: data.total_surebets ?? 0,
    active_bookmakers: parsers.filter((parser) => (parser.events ?? 0) > 0).length,
    failed_bookmakers: parsers.filter((parser) => Boolean(parser.error)).length,
    cache_hit_rate: data.cache_stats?.hit_rate ?? 0,
    memory_mb: data.performance?.memory?.current_mb ?? 0,
    timestamp: new Date().toISOString(),
  }
}

function mapCorridor(corridor: BackendCorridorOpportunity): CorridorOpportunity {
  const doubleWinProbability = corridor.double_win_probability
    ?? corridor.scenarios?.find((scenario) => scenario.both_win)?.probability
    ?? 0
  const expectedRoi = corridor.expected_roi ?? corridor.ev_percent ?? 0

  return {
    id: String(corridor.id),
    sport: String(corridor.sport),
    league: corridor.league ?? '',
    home_team: corridor.home_team,
    away_team: corridor.away_team,
    market: corridor.market,
    line_low: Math.min(corridor.line_a, corridor.line_b),
    line_high: Math.max(corridor.line_a, corridor.line_b),
    double_win_probability: doubleWinProbability,
    expected_roi: expectedRoi,
    legs: [
      {
        bookmaker: corridor.bookmaker_a,
        selection: `${corridor.market} ${corridor.line_a}`,
        odds: corridor.odds_a,
        line: corridor.line_a,
      },
      {
        bookmaker: corridor.bookmaker_b,
        selection: `${corridor.market} ${corridor.line_b}`,
        odds: corridor.odds_b,
        line: corridor.line_b,
      },
    ],
    detected_at: corridor.detected_at,
  }
}

async function fetchApiData<T>(path: string): Promise<T | null> {
  try {
    const response = await fetch(`${API_BASE}${path}`)
    if (!response.ok) return null

    const payload = await response.json() as ApiResponse<T> | T
    if (payload && typeof payload === 'object' && 'success' in payload) {
      const apiPayload = payload as ApiResponse<T>
      return apiPayload.success ? (apiPayload.data ?? null) : null
    }

    return payload as T
  } catch {
    return null
  }
}

export function useScanner() {
  const [connected, setConnected] = useState(false)
  const [surebets, setSurebets] = useState<Surebet[]>([])
  const [metrics, setMetrics] = useState<ScannerMetrics | null>(null)
  const [scannerStatus, setScannerStatus] = useState<ScannerStatus | null>(null)
  const [bookmakers, setBookmakers] = useState<Bookmaker[]>([])
  const [corridors, setCorridors] = useState<CorridorOpportunity[]>([])
  const [expressForks, setExpressForks] = useState<ExpressFork[]>([])
  const [valueBets, setValueBets] = useState<ValueBet[]>([])
  const [generosityIndices, setGenerosityIndices] = useState<GenerosityIndex[]>([])
  const [executionOverview, setExecutionOverview] = useState<ExecutionOverview | null>(null)
  const [executionLedger, setExecutionLedger] = useState<ExecutionLedgerAudit | null>(null)
  const [executionState, setExecutionState] = useState<ExecutionStateAudit | null>(null)
  const [parserCoverage, setParserCoverage] = useState<ParserCoverage[]>([])
  const [parserHealth, setParserHealth] = useState<ParserHealth[]>([])
  const [accounts, setAccounts] = useState<AccountStateResponse[]>([])
  const [accountsSummary, setAccountsSummary] = useState<AccountSessionSummary | null>(null)
  const [bankrollState, setBankrollState] = useState<BankrollState | null>(null)
  const [bankrollRecommendations, setBankrollRecommendations] = useState<BankrollRecommendationsResponse | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const notifiedIds = useRef<Set<string>>(new Set())
  const isFirstLoad = useRef(true)

  // Fetch real data from API
  const fetchRealData = useCallback(async () => {
    try {
      const [statusData, metricsData, bookmakersData, surebetsData, corridorsData, expressForksData, valueBetsData, generosityData, executionOverviewData, executionLedgerData, executionStateData, parserCoverageData, parserHealthData, accountsData, accountsSummaryData, bankrollStateData, bankrollRecommendationsData] = await Promise.all([
        fetchApiData<ScannerStatus>('/api/v1/scanner/status'),
        fetchApiData<ScannerMetrics>('/api/v1/metrics'),
        fetchApiData<BackendBookmaker[] | LegacyBookmakersResponse>('/api/v1/bookmakers'),
        fetchApiData<BackendSurebet[] | BackendCollectionResponse<BackendSurebet>>('/api/v1/surebets'),
        fetchApiData<BackendCorridorOpportunity[] | BackendCollectionResponse<BackendCorridorOpportunity>>('/api/v1/corridors'),
        fetchApiData<ExpressFork[]>('/api/v1/express-forks'),
        fetchApiData<ValueBet[]>('/api/v1/value-bets'),
        fetchApiData<BackendGenerosityIndex[] | { ranking?: BackendGenerosityIndex[] }>('/api/v1/analytics/generosity'),
        fetchApiData<ExecutionOverview>('/api/v1/execution/overview'),
        fetchApiData<ExecutionLedgerAudit>('/api/v1/execution/ledger?limit=25'),
        fetchApiData<ExecutionStateAudit>('/api/v1/execution/state?limit=25'),
        fetchApiData<ParserCoverage[]>('/api/v1/parsers/coverage'),
        fetchApiData<ParserHealth[]>('/api/v1/parsers/health'),
        fetchApiData<AccountStateResponse[]>('/api/v1/accounts'),
        fetchApiData<AccountSessionSummary>('/api/v1/accounts/summary'),
        fetchApiData<BankrollState>('/api/v1/bankroll'),
        fetchApiData<BankrollRecommendationsResponse>('/api/v1/bankroll/recommendations'),
      ])

      if (statusData) {
        setScannerStatus(statusData)
        if (statusData.last_metrics) {
          setMetrics(statusData.last_metrics)
        }
      }

      if (metricsData) {
        setMetrics(metricsData)
      }

      setBookmakers(normalizeBookmakersPayload(bookmakersData))

      setCorridors(unwrapCollection(corridorsData, 'corridors').map(mapCorridor))

      if (expressForksData) {
        setExpressForks(expressForksData)
      }

      if (valueBetsData) {
        setValueBets(valueBetsData)
      }

      setGenerosityIndices(normalizeGenerosityPayload(generosityData))

      if (executionOverviewData) {
        setExecutionOverview(executionOverviewData)
      }

      if (executionLedgerData) {
        setExecutionLedger(executionLedgerData)
      }

      if (executionStateData) {
        setExecutionState(executionStateData)
      }

      if (parserCoverageData) {
        setParserCoverage(parserCoverageData)
      }

      if (parserHealthData) {
        setParserHealth(parserHealthData)
      }

      if (accountsData) {
        setAccounts(accountsData)
      }

      if (accountsSummaryData) {
        setAccountsSummary(accountsSummaryData)
      }

      if (bankrollStateData) {
        setBankrollState(bankrollStateData)
      }

      if (bankrollRecommendationsData) {
        setBankrollRecommendations(bankrollRecommendationsData)
      }

      const normalizedSurebets = unwrapCollection(surebetsData, 'surebets').map(mapSurebet)
      if (normalizedSurebets.length > 0) {
        setSurebets(prev => {
          const newIds = new Set(normalizedSurebets.map(s => s.id))
          const merged = [...normalizedSurebets, ...prev.filter(s => !newIds.has(s.id))].slice(0, 500)
          merged.forEach((s) => notifiedIds.current.add(s.id))
          return merged
        })
      }

      if (isFirstLoad.current && (statusData || metricsData || bookmakersData || surebetsData || corridorsData || expressForksData || valueBetsData || generosityData || executionOverviewData || executionLedgerData || executionStateData || parserCoverageData || parserHealthData || accountsData || accountsSummaryData || bankrollStateData || bankrollRecommendationsData)) {
        isFirstLoad.current = false
        toast.success('Данные загружены с сервера')
      }
    } catch {
      // Silently fail and keep the last successful snapshot
    }
  }, [])

  const handleWsSurebet = useCallback((payload: unknown) => {
    if (!payload || typeof payload !== 'object') return

    const newSurebet = mapSurebet(payload as BackendSurebet)
    if (!notifiedIds.current.has(newSurebet.id)) {
      notifiedIds.current.add(newSurebet.id)
      setSurebets(prev => [newSurebet, ...prev].slice(0, 1000))
      toast.success(
        `Вилка +${newSurebet.profit_percent.toFixed(2)}%`,
        { description: `${newSurebet.home_team} vs ${newSurebet.away_team}`, duration: 5000 }
      )
    }
  }, [])

  const handleWsMetrics = useCallback((payload: unknown) => {
    const nextMetrics = normalizeMetricsPayload(payload)
    if (!nextMetrics) return

    setMetrics(nextMetrics)
    setScannerStatus(prev => prev ? { ...prev, last_metrics: nextMetrics } : prev)
  }, [])

  const handleWsMessage = useCallback((payload: unknown) => {
    if (!payload || typeof payload !== 'object') return

    const compatMessage = payload as CompatWsMessage
    const messageType = compatMessage.type ?? compatMessage.event

    if (messageType === 'new_surebet' || messageType === 'surebet.created') {
      handleWsSurebet(compatMessage.data)
      return
    }

    if (messageType === 'surebets' && Array.isArray(compatMessage.data)) {
      const normalizedSurebets = compatMessage.data.map((surebet) => mapSurebet(surebet as BackendSurebet))
      setSurebets(normalizedSurebets)
      return
    }

    if (messageType === 'stats' || messageType === 'stats_update' || messageType === 'scanner.metrics') {
      handleWsMetrics(compatMessage.data)
      return
    }

    const rawBusMessage = payload as RawSurebetBusMessage
    if (rawBusMessage.SurebetFound?.payload) {
      handleWsSurebet(rawBusMessage.SurebetFound.payload)
    }
  }, [handleWsMetrics, handleWsSurebet])

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return

    try {
      const ws = new WebSocket(WS_URL)
      wsRef.current = ws

      ws.onopen = () => {
        setConnected(true)
        console.log('[WS] Connected to backend')
        toast.success('Подключено к сканеру')
        // Fetch real data immediately
        fetchRealData()
      }

      ws.onmessage = (event) => {
        try {
          handleWsMessage(JSON.parse(event.data) as unknown)
        } catch { /* ignore */ }
      }

      ws.onclose = () => {
        setConnected(false)
        reconnectTimer.current = setTimeout(connect, 5000)
      }

      ws.onerror = () => ws.close()
    } catch {
      reconnectTimer.current = setTimeout(connect, 5000)
    }
  }, [fetchRealData, handleWsMessage])

  useEffect(() => {
    connect()
    // Poll API every 30s as fallback
    const interval = setInterval(fetchRealData, 30000)
    return () => {
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current)
      wsRef.current?.close()
      clearInterval(interval)
    }
  }, [connect, fetchRealData])

  return {
    connected,
    scannerStatus,
    surebets,
    metrics,
    bookmakers,
    corridors,
    expressForks,
    valueBets,
    generosityIndices,
    executionOverview,
    executionLedger,
    executionState,
    parserCoverage,
    parserHealth,
    accounts,
    accountsSummary,
    bankrollState,
    bankrollRecommendations,
  }
}
