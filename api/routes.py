# api/routes.py
from fastapi import APIRouter, HTTPException, Query, Body
from typing import List, Optional, Any
from pydantic import BaseModel

router = APIRouter()

scanner = None


def set_scanner(s):
    global scanner
    scanner = s


class ApiResponse(BaseModel):
    success: bool
    data: Optional[Any] = None
    error: Optional[str] = None


class BankrollUpdateRequest(BaseModel):
    bookmaker: str
    balance: float
    currency: str = "RUB"
    initial_balance: Optional[float] = None


@router.get("/api/v1/analytics/summary")
async def get_analytics_summary():
    from services.analytics import analytics_engine
    return ApiResponse(success=True, data=analytics_engine.get_summary())


@router.get("/api/v1/analytics/history")
async def get_analytics_history(
    limit: int = Query(50, ge=1, le=200),
    hours: Optional[int] = None
):
    from services.analytics import analytics_engine
    return ApiResponse(success=True, data=analytics_engine.get_history(limit=limit, hours=hours))


@router.get("/api/v1/analytics/chart")
async def get_profit_chart(hours: int = Query(24, ge=1, le=168)):
    from services.analytics import analytics_engine
    return ApiResponse(success=True, data=analytics_engine.get_profit_chart_data(hours=hours))


@router.get("/api/v1/analytics/bookmakers")
async def get_bookmaker_comparison():
    from services.analytics import analytics_engine
    return ApiResponse(success=True, data=analytics_engine.get_bookmaker_comparison())


@router.get("/api/v1/analytics/export")
async def export_analytics():
    from services.analytics import analytics_engine
    return ApiResponse(success=True, data=analytics_engine.export_data())


@router.get("/api/v1/analytics/generosity")
async def get_generosity_index(sport: Optional[str] = None):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    if sport:
        ranking = scanner.generosity_index.get_ranking(sport=sport)
        best = scanner.generosity_index.get_best_for_sport(sport)
        return ApiResponse(success=True, data={
            'sport': sport,
            'ranking': ranking,
            'best_bookmaker': best,
        })
    
    summary = scanner.generosity_index.get_summary()
    return ApiResponse(success=True, data=summary)


@router.get("/api/v1/surebets")
async def get_surebets(
    min_profit: float = Query(0.5, ge=0, le=100),
    sport: Optional[str] = None,
    live_only: bool = False,
    limit: int = Query(50, ge=1, le=200)
):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    surebets = scanner.get_surebets(min_profit=min_profit)
    
    if sport:
        surebets = [s for s in surebets if s.get('sport') == sport]
    
    if live_only:
        surebets = [s for s in surebets if s.get('is_live')]
    
    return ApiResponse(
        success=True,
        data={"surebets": surebets[:limit], "total": len(surebets)}
    )


@router.get("/api/v1/surebets/top")
async def get_top_surebets(limit: int = Query(10, ge=1, le=50)):
    if not scanner:
        return {"surebets": []}
    return {"surebets": scanner.get_top_surebets(limit)}


@router.get("/api/v1/events")
async def get_events(
    bookmaker: Optional[str] = None,
    sport: Optional[str] = None,
    live_only: bool = False,
    limit: int = Query(100, ge=1, le=500)
):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    events = scanner.get_events()
    
    if bookmaker:
        events = [e for e in events if e.get('bookmaker') == bookmaker]
    
    if sport:
        events = [e for e in events if e.get('sport') == sport]
    
    if live_only:
        events = [e for e in events if e.get('is_live')]
    
    return ApiResponse(
        success=True,
        data={"events": events[:limit], "total": len(events)}
    )


@router.get("/api/v1/stats")
async def get_stats():
    if not scanner:
        return {
            "scanner_running": False,
            "events_count": 0,
            "surebets_count": 0,
            "value_bets_count": 0,
            "corridors_count": 0,
            "last_scan": None
        }
    
    stats = scanner.get_stats()
    return {
        "scanner_running": stats.get('is_running', False),
        "events_count": stats.get('total_events', 0),
        "surebets_count": stats.get('total_surebets', 0),
        "value_bets_count": stats.get('total_value_bets', 0),
        "corridors_count": stats.get('total_corridors', 0),
        "last_cycle_ms": stats.get('last_cycle_time_ms', 0),
        "avg_cycle_ms": stats.get('avg_cycle_time_ms', 0),
        "cache_hit_rate": stats.get('cache_stats', {}).get('hit_rate', 0),
        "parsers": stats.get('parsers', {}),
        "sources": list(stats.get('sources', [])),
        "value_detector": stats.get('value_detector', {}),
        "corridor_finder": stats.get('corridor_finder', {}),
        "reliability": stats.get('reliability', {}),
    }


@router.get("/api/v1/bookmakers")
async def get_bookmakers():
    from models.bookmaker import BOOKMAKERS
    return {
        "bookmakers": [
            {
                "id": bk.id,
                "name": bk.name,
                "slug": bk.slug,
                "url_live": bk.url_live,
                "priority": bk.priority
            }
            for bk in BOOKMAKERS.values()
        ]
    }


@router.get("/api/v1/bonuses")
async def get_bonuses():
    if scanner and hasattr(scanner, 'freebet_hunter'):
        freebets = scanner.freebet_hunter.get_available_freebets()
        return {"bonuses": freebets}
    return {
        "bonuses": [
            {"id": "winline", "name": "Winline", "bonus": "100% до 10,000₽", "conditions": "Вейджер x10"},
            {"id": "olimp", "name": "Olimp", "bonus": "Фрибет 500₽", "conditions": "Экспресс 3+"},
            {"id": "pari", "name": "Pari", "bonus": "100% до 15,000₽", "conditions": "Вейджер x10"},
            {"id": "fonbet", "name": "Fonbet", "bonus": "Фрибет 2,000₽", "conditions": "Экспресс 3+"},
            {"id": "1xbet", "name": "1xBet", "bonus": "3,000₽", "conditions": "Вейджер x5"},
            {"id": "marathon", "name": "Marathon", "bonus": "Кэшбэк", "conditions": "Еженедельно"},
            {"id": "betboom", "name": "BetBoom", "bonus": "100% до 20,000₽", "conditions": "Вейджер x8"},
            {"id": "leon", "name": "Leon", "bonus": "Фрибет 500₽", "conditions": "За регистрацию"},
        ]
    }


@router.get("/api/v1/freebets")
async def get_freebets():
    if not scanner or not hasattr(scanner, 'freebet_hunter'):
        return ApiResponse(success=False, error="Freebet hunter not available")
    
    return ApiResponse(success=True, data={
        'available_freebets': scanner.freebet_hunter.get_available_freebets(),
        'best_strategy': scanner.freebet_hunter.get_best_freebet_strategy(scanner.surebets) if scanner.surebets else None,
    })


@router.get("/api/v1/freebets/surebets")
async def get_freebet_surebets(
    total_stake: float = Query(10000, ge=100, le=1000000),
    min_roi: float = Query(5.0, ge=0.1, le=100)
):
    if not scanner or not hasattr(scanner, 'freebet_hunter'):
        return ApiResponse(success=False, error="Freebet hunter not available")
    
    scanner.freebet_hunter.min_freebet_roi = min_roi
    freebet_surebets = scanner.freebet_hunter.find_freebet_surebets(scanner.surebets, total_stake)
    
    return ApiResponse(success=True, data={
        'count': len(freebet_surebets),
        'surebets': freebet_surebets[:50],
        'total_stake': total_stake,
        'min_roi': min_roi,
    })


class SettingsUpdateRequest(BaseModel):
    min_profit: Optional[float] = None
    cycle_interval: Optional[float] = None
    max_events_per_source: Optional[int] = None
    cache_ttl: Optional[float] = None
    enabled_sources: Optional[list] = None
    live_only: Optional[bool] = None
    prematch_enabled: Optional[bool] = None
    cyber_enabled: Optional[bool] = None
    min_odds: Optional[float] = None
    max_odds: Optional[float] = None


@router.get("/api/v1/settings")
async def get_settings():
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    return ApiResponse(success=True, data=scanner.get_config())


@router.post("/api/v1/settings")
async def update_settings(request: SettingsUpdateRequest):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    updates = {}
    if request.min_profit is not None:
        updates['min_profit'] = request.min_profit
    if request.cycle_interval is not None:
        updates['cycle_interval'] = request.cycle_interval
    if request.max_events_per_source is not None:
        updates['max_events_per_source'] = request.max_events_per_source
    if request.cache_ttl is not None:
        updates['cache_ttl'] = request.cache_ttl
    if request.enabled_sources is not None:
        updates['enabled_sources'] = set(request.enabled_sources)
    if request.live_only is not None:
        updates['live_only'] = request.live_only
    if request.prematch_enabled is not None:
        updates['prematch_enabled'] = request.prematch_enabled
    if request.cyber_enabled is not None:
        updates['cyber_enabled'] = request.cyber_enabled
    if request.min_odds is not None:
        updates['min_odds'] = request.min_odds
    if request.max_odds is not None:
        updates['max_odds'] = request.max_odds
    
    new_config = scanner.update_config(**updates)
    
    return ApiResponse(success=True, data=new_config)


@router.get("/api/v1/sources/available")
async def get_available_sources():
    return ApiResponse(success=True, data={
        'working': ['winline', 'pari', 'betcity', 'marathon', 'zenit'],
        'blocked': ['1xstavka', 'pinup', 'betboom', 'fonbet', 'leon', 'olimp', 'olimpbet'],
    })


@router.post("/api/v1/sources/{slug}/toggle")
async def toggle_source(slug: str, enabled: bool = True):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    sources = set(scanner.config.enabled_sources)
    
    if enabled:
        sources.add(slug)
    else:
        sources.discard(slug)
    
    scanner.update_config(enabled_sources=sources)
    
    return ApiResponse(success=True, data={
        'slug': slug,
        'enabled': enabled,
        'all_sources': list(scanner.config.enabled_sources)
    })


@router.post("/api/v1/scanner/start")
async def start_scanner():
    if scanner and not scanner.is_running:
        await scanner.start()
    return {"status": "started"}


@router.post("/api/v1/scanner/stop")
async def stop_scanner():
    if scanner and scanner.is_running:
        await scanner.stop()
    return {"status": "stopped"}


@router.get("/api/v1/calculator")
async def calculate_surebet(
    odds: str = Query(..., description="Коэффициенты через запятую"),
    stake: float = Query(10000, gt=0)
):
    try:
        odds_list = [float(o.strip()) for o in odds.split(',') if o.strip()]
        
        if len(odds_list) < 2:
            return ApiResponse(success=False, error="Need at least 2 odds")
        
        inverses = [1/o for o in odds_list]
        sum_inv = sum(inverses)
        
        if sum_inv >= 1:
            return ApiResponse(
                success=True,
                data={
                    "is_surebet": False,
                    "margin_percent": (sum_inv - 1) * 100,
                    "message": "Not a surebet"
                }
            )
        
        profit = (1/sum_inv - 1) * 100
        stakes = [stake * inv / sum_inv for inv in inverses]
        
        return ApiResponse(
            success=True,
            data={
                "is_surebet": True,
                "profit_percent": profit,
                "estimated_profit": stake * profit / 100,
                "stakes": [
                    {"odds": o, "stake": s, "percent": s/stake*100}
                    for o, s in zip(odds_list, stakes)
                ],
                "total_stake": stake
            }
        )
    except Exception as e:
        return ApiResponse(success=False, error=str(e))


@router.get("/api/v1/search")
async def search(query: str = Query(..., min_length=2)):
    if not scanner:
        return {"results": []}
    
    results = [s for s in scanner.surebets if query.lower() in s.get('event_name', '').lower()]
    return {"results": results[:20]}


@router.get("/api/v1/valuebets")
async def get_value_bets(
    min_edge: float = Query(2.0, ge=0, le=50),
    sport: Optional[str] = None,
    bookmaker: Optional[str] = None,
    limit: int = Query(50, ge=1, le=200)
):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    value_bets = scanner.value_detector.find_value_bets(
        events=scanner.get_events(),
        min_edge=min_edge,
        sport=sport,
        bookmaker=bookmaker,
    )
    
    return ApiResponse(
        success=True,
        data={"valuebets": value_bets[:limit], "total": len(value_bets)}
    )


@router.get("/api/v1/valuebets/stats")
async def get_value_bets_stats():
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    return ApiResponse(
        success=True,
        data=scanner.value_detector.get_stats()
    )


@router.get("/api/v1/corridors")
async def get_corridors(
    min_ev: float = Query(1.0, ge=0, le=50),
    sport: Optional[str] = None,
    limit: int = Query(50, ge=1, le=200)
):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    corridors = scanner.corridor_finder.find_corridors(
        events=scanner.get_events(),
        min_ev=min_ev,
        sport=sport,
    )
    
    return ApiResponse(
        success=True,
        data={"corridors": corridors[:limit], "total": len(corridors)}
    )


@router.get("/api/v1/bankroll")
async def get_bankroll():
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    from services.bankroll import BankrollManager
    bm = scanner.bankroll_manager
    return ApiResponse(success=True, data=bm.get_summary())


@router.post("/api/v1/bankroll/update")
async def update_bankroll(request: BankrollUpdateRequest):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    from services.bankroll import BankrollManager
    bm = scanner.bankroll_manager
    account = await bm.update_balance(
        bookmaker=request.bookmaker,
        balance=request.balance,
        currency=request.currency,
        initial_balance=request.initial_balance,
    )
    
    return ApiResponse(success=True, data=account.to_dict())


@router.get("/api/v1/bankroll/optimal")
async def get_optimal_distribution(
    total_stake: float = Query(10000, gt=0),
    risk_level: Optional[str] = Query(None)
):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    from services.bankroll import BankrollManager, RiskLevel
    bm = scanner.bankroll_manager
    
    risk = None
    if risk_level:
        try:
            risk = RiskLevel(risk_level.lower())
        except ValueError:
            return ApiResponse(success=False, error=f"Invalid risk level: {risk_level}")
    
    distribution = bm.calculate_optimal_distribution(
        total_amount=total_stake,
        risk_level=risk,
    )
    
    return ApiResponse(
        success=True,
        data={
            "total_stake": total_stake,
            "risk_level": risk_level or bm.risk_level.value,
            "distribution": [d.to_dict() for d in distribution],
        }
    )


@router.get("/api/v1/bookmakers/{slug}/reliability")
async def get_bookmaker_reliability(slug: str):
    if not scanner:
        return ApiResponse(success=False, error="Scanner not initialized")
    
    score = scanner.reliability_scorer.calculate_score(slug)
    return ApiResponse(success=True, data=score.to_dict())


@router.get("/api/v1/ghost/accounts")
async def get_ghost_accounts():
    from core.ghost_mode import ghost_mode
    heats = ghost_mode.get_all_heats()
    return ApiResponse(success=True, data={
        'accounts': heats,
        'total_tracked': len(heats)
    })


@router.post("/api/v1/ghost/record")
async def record_bet_result(data: dict):
    from core.ghost_mode import ghost_mode
    
    bookmaker = data.get('bookmaker', '')
    won = data.get('won', False)
    amount = float(data.get('amount', 0))
    
    ghost_mode.record_bet_result(bookmaker, won, amount)
    
    return ApiResponse(success=True, data={
        'bookmaker': bookmaker,
        'heat_level': ghost_mode.get_account_heat(bookmaker),
        'should_slow_down': ghost_mode.should_slow_down(bookmaker)
    })


@router.get("/api/v1/ghost/recommendation")
async def get_ghost_recommendation(bookmaker: str):
    from core.ghost_mode import ghost_mode
    
    heat = ghost_mode.get_account_heat(bookmaker)
    delay = ghost_mode.get_delay_seconds(bookmaker)
    should_slow = ghost_mode.should_slow_down(bookmaker)
    remaining = ghost_mode.get_remaining_budget(bookmaker)
    
    return ApiResponse(success=True, data={
        'bookmaker': bookmaker,
        'heat_level': heat,
        'recommended_delay_seconds': delay,
        'should_slow_down': should_slow,
        'remaining_budget': remaining,
        'status': 'HOT' if heat > 70 else 'WARM' if heat > 40 else 'COOL'
    })


@router.get("/api/v1/live/stats")
async def get_live_stats():
    from scanner.live_scanner import live_scanner
    
    stats = live_scanner.get_stats()
    return ApiResponse(success=True, data=stats)


@router.post("/api/v1/live/start")
async def start_live():
    from scanner.live_scanner import live_scanner
    
    await live_scanner.start()
    return ApiResponse(success=True, data={'status': 'started'})


@router.post("/api/v1/live/stop")
async def stop_live():
    from scanner.live_scanner import live_scanner
    
    await live_scanner.stop()
    return ApiResponse(success=True, data={'status': 'stopped'})


@router.get("/api/v1/proxy/stats")
async def get_proxy_stats():
    from core.proxy_manager import proxy_manager
    
    stats = proxy_manager.get_stats()
    return ApiResponse(success=True, data=stats)


@router.post("/api/v1/proxy/test")
async def test_proxies():
    from core.proxy_manager import proxy_manager
    
    results = await proxy_manager.test_proxies()
    return ApiResponse(success=True, data={
        'results': results,
        'summary': proxy_manager.get_stats()
    })


@router.get("/api/v1/bet/pending")
async def get_pending_bets():
    from automation.auto_better import auto_better
    
    pending = auto_better.get_pending()
    return ApiResponse(success=True, data={
        'pending': [
            {
                'surebet_id': p.surebet_id,
                'bookmaker': p.bookmaker,
                'selection': p.selection,
                'odds': p.odds,
                'stake': p.stake,
                'event_name': p.event_name
            }
            for p in pending
        ],
        'total': len(pending)
    })


@router.post("/api/v1/bet/confirm/{surebet_id}")
async def confirm_bet(surebet_id: str):
    from automation.auto_better import auto_better
    
    success = auto_better.confirm_bet(surebet_id)
    return ApiResponse(success=success, data={'confirmed': success})


@router.post("/api/v1/bet/cancel/{surebet_id}")
async def cancel_bet(surebet_id: str):
    from automation.auto_better import auto_better
    
    success = auto_better.cancel_bet(surebet_id)
    return ApiResponse(success=success, data={'cancelled': success})


@router.get("/api/v1/bet/stats")
async def get_bet_stats():
    from automation.auto_better import auto_better
    
    return ApiResponse(success=True, data=auto_better.get_stats())


@router.post("/api/v1/bet/mode")
async def set_bet_mode(data: dict):
    from automation.auto_better import auto_better
    from automation.auto_better import BetMode
    
    mode = data.get('mode', 'manual')
    modes = {
        'manual': BetMode.MANUAL,
        'semi_auto': BetMode.SEMI_AUTO,
        'full_auto': BetMode.FULL_AUTO
    }
    
    if mode in modes:
        auto_better.set_mode(modes[mode])
        return ApiResponse(success=True, data={'mode': mode})
    
    return ApiResponse(success=False, error='Invalid mode')


@router.get("/api/v1/bookmarklets")
async def get_bookmarklets():
    from automation.bookmarklets import BookmarkletGenerator
    
    html = BookmarkletGenerator.generate_html_page([])
    return ApiResponse(success=True, data={'html': html})
