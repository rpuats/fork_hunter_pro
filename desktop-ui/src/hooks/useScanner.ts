import { useState, useEffect, useCallback, useRef } from 'react'
import { toast } from 'sonner'
import type { Surebet, ScannerMetrics, Bookmaker, CorridorOpportunity, ExpressFork } from '../types'
import { mockSurebets, mockBookmakers, mockMetrics, mockCorridors, mockExpressForks } from '../data/mockData'

const WS_URL = 'ws://localhost:8080/ws'
const API_BASE = 'http://localhost:8080'

export function useScanner() {
  const [connected, setConnected] = useState(false)
  const [surebets, setSurebets] = useState<Surebet[]>(mockSurebets)
  const [metrics, setMetrics] = useState<ScannerMetrics | null>(mockMetrics)
  const [bookmakers, setBookmakers] = useState<Bookmaker[]>(mockBookmakers)
  const [corridors, setCorridors] = useState<CorridorOpportunity[]>(mockCorridors)
  const [expressForks, setExpressForks] = useState<ExpressFork[]>(mockExpressForks)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const notifiedIds = useRef<Set<string>>(new Set(mockSurebets.map(s => s.id)))
  const isFirstLoad = useRef(true)

  // Fetch real data from API
  const fetchRealData = useCallback(async () => {
    try {
      const [metricsRes, bookmakersRes, surebetsRes] = await Promise.all([
        fetch(`${API_BASE}/api/v1/metrics`).catch(() => null),
        fetch(`${API_BASE}/api/v1/bookmakers`).catch(() => null),
        fetch(`${API_BASE}/api/v1/surebets`).catch(() => null),
      ])

      if (metricsRes?.ok) {
        const data = await metricsRes.json()
        if (data.data) {
          setMetrics(data.data)
          if (isFirstLoad.current) {
            isFirstLoad.current = false
            toast.success('Данные загруены с сервера')
          }
        }
      }

      if (bookmakersRes?.ok) {
        const data = await bookmakersRes.json()
        if (data.data?.length > 0) {
          setBookmakers(data.data.map((bk: any) => ({
            name: bk.name,
            slug: bk.slug,
            status: bk.status || 'active',
            events: bk.events || 0,
            odds: bk.events * 25,
            last_update: new Date().toISOString(),
          })))
        }
      }

      if (surebetsRes?.ok) {
        const data = await surebetsRes.json()
        if (data.data?.length > 0) {
          setSurebets(prev => {
            const newIds = new Set(data.data.map((s: Surebet) => s.id))
            const merged = [...data.data, ...prev.filter(s => !newIds.has(s.id))].slice(0, 500)
            merged.forEach((s: Surebet) => notifiedIds.current.add(s.id))
            return merged
          })
        }
      }
    } catch (e) {
      // Silently fail — mock data is used as fallback
    }
  }, [])

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
          const data = JSON.parse(event.data)
          if (data.type === 'surebet_found') {
            const newSurebet = data.data as Surebet
            if (!notifiedIds.current.has(newSurebet.id)) {
              notifiedIds.current.add(newSurebet.id)
              setSurebets(prev => [newSurebet, ...prev].slice(0, 1000))
              toast.success(
                `Вилка +${newSurebet.profitPercent.toFixed(2)}%`,
                { description: `${newSurebet.home_team} vs ${newSurebet.away_team}`, duration: 5000 }
              )
            }
          } else if (data.type === 'metrics') {
            setMetrics(data.data)
          }
        } catch (e) { /* ignore */ }
      }

      ws.onclose = () => {
        setConnected(false)
        reconnectTimer.current = setTimeout(connect, 5000)
      }

      ws.onerror = () => ws.close()
    } catch (e) {
      reconnectTimer.current = setTimeout(connect, 5000)
    }
  }, [fetchRealData])

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

  return { connected, surebets, metrics, bookmakers, corridors, expressForks }
}
