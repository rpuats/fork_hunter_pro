export interface Surebet {
  id: string
  sport: string
  league: string
  home_team: string
  away_team: string
  start_time: string | null
  is_live: boolean
  profit_percent: number
  profitPercent: number
  total_stake: number
  legs: SurebetLeg[]
  detected_at: string
  verified: boolean
  mirror: boolean
}

export interface SurebetLeg {
  bookmaker: string
  market: string
  selection: string
  odds: number
  line: number | null
  stake: number
  payout: number
  url: string | null
}

export interface ScannerMetrics {
  cycle_time_ms: number
  events_parsed: number
  surebets_found: number
  active_bookmakers: number
  failed_bookmakers: number
  cache_hit_rate: number
  memory_mb: number
  timestamp: string
}

export interface Bookmaker {
  name: string
  slug: string
  status: 'active' | 'inactive' | 'error'
  events: number
  odds: number
  last_update: string | null
}

export interface CorridorOpportunity {
  id: string
  sport: string
  league: string
  home_team: string
  away_team: string
  market: string
  line_low: number
  line_high: number
  double_win_probability: number
  expected_roi: number
  legs: CorridorLeg[]
  detected_at: string
}

export interface CorridorLeg {
  bookmaker: string
  selection: string
  odds: number
  line: number
}

export interface ExpressFork {
  id: string
  profit_percent: number
  total_stake: number
  legs: ExpressForkLeg[]
  risk: 'low' | 'medium' | 'high'
  detected_at: string
}

export interface ExpressForkLeg {
  event: string
  market: string
  selection: string
  odds: number
  bookmaker: string
}

export type TabType = 'dashboard' | 'surebets' | 'corridors' | 'express' | 'history' | 'settings'
