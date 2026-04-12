# scanner/engine.py
import asyncio
import time
import hashlib
from datetime import datetime
from typing import List, Dict, Optional, Callable
from dataclasses import dataclass, field
import logging
from collections import defaultdict

from core.cache import event_cache, surebet_cache, rate_limiter, AsyncTTLCache
from core.normalizer import event_normalizer
from core.finder import SurebetCalculator
from core.finder_optimized import OptimizedSurebetCalculator, ParallelSurebetDetector
from core.momentum_scanner import MomentumScanner
from core.freebet_hunter import FreebetHunter
from core.value_detector import ValueBetDetector
from core.corridor_finder import CorridorFinder
from core.event_pool import EventPool
from core.performance import PerformanceMonitor
from core.memory_manager import memory_manager, ObjectPool
from core.connection_pool import connection_pool, SharedConnectionPool
from core.generosity_index import BookmakerGenerosityIndex
from core.odds_verifier import OddsVerifier
from core.mirror_detector import MirrorLineDetector
from core.odds_error_detector import OddsErrorDetector
from core.surebet_history import SurebetHistory
from scanner.parsers import ALL_PARSERS
from services.database import Database
from services.reliability import ReliabilityScorer
from services.bankroll import BankrollManager

logger = logging.getLogger(__name__)


WORKING_SOURCES = {
    'winline', 'pari', 'betcity', 'marathon', 'zenit',
    'bettery', 'baltbet', 'betboom', 'fonbet',
    'ligastavok', 'sportbet', '24bet', 'betboo',
}

BLOCKED_SOURCES = {
    '1xstavka',  # Geo-blocked
    'leon',       # API requires auth
    'pinup',      # DNS fail
    'olimp',      # SPA
    'olimpbet',   # SPA
    'tennisi',    # Timeout
    'betm',       # Timeout
    'melbet',     # Cloudflare/SPA
    'ligastavok', # DDoS-Guard
    'sportbet',   # DDoS-Guard
    'betboo',     # Site doesn't exist
}


@dataclass
class ScannerConfig:
    min_profit: float = 0.5
    cycle_interval: float = 3.0
    max_events_per_source: int = 200
    cache_ttl: float = 10.0
    enabled_sources: set = field(default_factory=lambda: WORKING_SOURCES)
    live_only: bool = False
    prematch_enabled: bool = True
    cyber_enabled: bool = True
    min_odds: float = 1.01
    max_odds: float = 50.0


@dataclass
class ScannerStats:
    total_cycles: int = 0
    total_events: int = 0
    total_surebets: int = 0
    total_value_bets: int = 0
    total_corridors: int = 0
    total_odds_errors: int = 0
    verified_count: int = 0
    expired_count: int = 0
    cache_hits: int = 0
    cache_misses: int = 0
    parser_stats: Dict = field(default_factory=dict)
    last_cycle_time: Optional[float] = None
    avg_cycle_time: float = 0.0
    uptime_seconds: float = 0.0
    started_at: Optional[float] = None
    surebets_per_cycle: float = 0.0
    events_per_cycle: float = 0.0


class GhostScanner:
    """
    High-performance fork scanner with:
    - Parallel parsing of all bookmakers
    - Smart caching with TTL
    - Rate limiting per source
    - Event normalization
    - Real-time updates
    - Lock-free event pool
    - Vectorized surebet detection
    - Memory-bounded storage
    """
    
    def __init__(self, database: Database, config: Optional[ScannerConfig] = None, use_optimized: bool = True):
        self.database = database
        self.config = config or ScannerConfig()
        self.use_optimized = use_optimized
        
        if use_optimized:
            self.calculator = OptimizedSurebetCalculator(min_profit=self.config.min_profit)
            self.parallel_detector = ParallelSurebetDetector(min_profit=self.config.min_profit)
        else:
            self.calculator = SurebetCalculator(min_profit=self.config.min_profit)
            self.parallel_detector = None
        
        self.value_detector = ValueBetDetector(min_edge=2.0)
        self.corridor_finder = CorridorFinder(min_ev=1.0)
        self.reliability_scorer = ReliabilityScorer()
        self.bankroll_manager = BankrollManager(db_path=database.path)
        self.freebet_hunter = FreebetHunter(min_freebet_roi=5.0)
        self.momentum_scanner = MomentumScanner(min_profit=5.0)
        
        self.value_bets: List[Dict] = []
        self.corridors: List[Dict] = []
        self.freebet_surebets: List[Dict] = []
        
        self.generosity_index = BookmakerGenerosityIndex()
        
        self.odds_verifier = OddsVerifier(tolerance=0.02, min_profit=self.config.min_profit)
        
        self.mirror_detector = MirrorLineDetector(mirror_threshold=0.95, independent_threshold=0.80)
        
        self.odds_error_detector = OddsErrorDetector(default_threshold=0.25, min_bookmakers=3)
        
        self.odds_errors: List[Dict] = []
        
        self.surebet_history = SurebetHistory(db_path=database.path)
        
        self.perf_monitor = PerformanceMonitor()
        self.perf_monitor.start_tracemalloc()
        
        memory_manager.start()
        
        self.parsers = self._init_parsers()
        
        self.event_pool = EventPool(
            max_size=50000,
            eviction_threshold=0.9,
            stale_ttl=300.0,
        )
        
        self.events_cache: AsyncTTLCache = AsyncTTLCache(
            maxsize=50000,
            default_ttl=self.config.cache_ttl
        )
        
        self.events: Dict[str, List[Dict]] = defaultdict(list)
        self.surebets: List[Dict] = []
        self.seen_surebet_ids = EventPool(max_size=10000, stale_ttl=600.0)
        
        self.is_running = False
        self.stats = ScannerStats()
        
        self._subscribers: List[Callable] = []
        self._telegram_callback: Optional[Callable] = None
        
        self._cycle_times: List[float] = []
        self._max_cycle_times = 100
        
        self._previous_events_hash: str = ""
        self._last_events_snapshot: Dict[str, float] = {}
        
        self._dict_pool = ObjectPool(factory=dict, max_size=1000, name="event_dict")
        memory_manager.register_pool("event_dict", self._dict_pool)
        
        self._shared_browser = None
        self._shared_playwright = None
    
    async def _get_shared_browser(self):
        if self._shared_browser is None:
            from playwright.async_api import async_playwright
            if self._shared_playwright is None:
                self._shared_playwright = await async_playwright().start()
            self._shared_browser = await self._shared_playwright.chromium.launch(
                headless=True,
                args=['--disable-blink-features=AutomationControlled']
            )
        return self._shared_browser
    
    async def _close_shared_browser(self):
        if self._shared_browser:
            try:
                await self._shared_browser.close()
            except:
                pass
            self._shared_browser = None
        if self._shared_playwright:
            try:
                await self._shared_playwright.stop()
            except:
                pass
            self._shared_playwright = None
    
    def _init_parsers(self) -> List:
        import os
        use_mock = os.getenv("USE_MOCK_DATA", "false").lower() == "true"
        
        parsers = []
        
        # Working parsers - Playwright
        playwright_parsers = {
            'winline': ('scanner.parsers.winline_playwright', 'WinlinePlaywrightParser'),
            'pari': ('scanner.parsers.pari_playwright', 'PariPlaywrightParser'),
            'marathon': ('scanner.parsers.marathon_playwright', 'MarathonPlaywrightParser'),
            'zenit': ('scanner.parsers.zenit_playwright', 'ZenitPlaywrightParser'),
            'bettery': ('scanner.parsers.bettery_playwright', 'BetteryPlaywrightParser'),
            'baltbet': ('scanner.parsers.baltbet_playwright', 'BaltbetRegexParser'),
            'betboom': ('scanner.parsers.betboom_playwright', 'BetBoomPlaywrightParser'),
            'fonbet': ('scanner.parsers.fonbet_playwright', 'FonbetPlaywrightParser'),
            '24bet': ('scanner.parsers._24bet_playwright', '_24betPlaywrightParser'),
            'olimpbet': ('scanner.parsers.olimpbet_playwright', 'OlimpBetPlaywrightParser'),
        }
        
        # Working API parsers (no Playwright needed)
        api_parsers = {
            'leon': ('scanner.parsers.leon_api', 'LeonApiParser'),
        }
        
        if use_mock:
            from scanner.parsers.mock_parser import MOCK_PARSERS
            for parser_class in MOCK_PARSERS:
                if parser_class.slug in self.config.enabled_sources:
                    parsers.append(parser_class())
            logger.info("🎭 Using MOCK parsers for testing")
        else:
            # Try Playwright parsers first
            for slug, (module_name, class_name) in playwright_parsers.items():
                if slug in self.config.enabled_sources:
                    try:
                        module = __import__(module_name, fromlist=[class_name])
                        cls = getattr(module, class_name)
                        parsers.append(cls())
                        logger.info(f"✅ {slug}: {class_name}")
                    except Exception as e:
                        logger.warning(f"❌ {slug}: Failed to load Playwright parser: {e}")
            
            # Add HTTP parsers for remaining sources (non-Playwright)
            for parser_class in ALL_PARSERS:
                slug = getattr(parser_class, 'slug', '')
                if slug in self.config.enabled_sources and slug not in playwright_parsers:
                    try:
                        parsers.append(parser_class())
                    except Exception as e:
                        logger.warning(f"❌ {slug}: Failed to load parser: {e}")
        
        logger.info(f"Initialized {len(parsers)} parsers: {[p.name for p in parsers]}")
        return parsers
    
    async def start(self):
        if self.is_running:
            return
        
        await self.database.init()
        await self.bankroll_manager.init()
        await self.surebet_history.init()
        
        await self.momentum_scanner.initialize()
        self.momentum_scanner.hook_into_engine(self)
        await self.momentum_scanner.start()
        
        self.is_running = True
        self.stats.started_at = time.time()
        logger.info("🎯 Ghost Scanner started")
        
        asyncio.create_task(self._run_loop())
        asyncio.create_task(self._cleanup_loop())
        asyncio.create_task(self._stats_loop())
    
    async def stop(self):
        self.is_running = False
        
        for parser in self.parsers:
            try:
                await parser.close()
            except:
                pass
        
        await self.surebet_history.close()
        await self._close_shared_browser()
        
        await self.momentum_scanner.stop()
        
        if self.parallel_detector:
            self.parallel_detector.shutdown()
        
        await connection_pool.close()
        memory_manager.stop()
        self.perf_monitor.stop_tracemalloc()
        logger.info("⏹️ Ghost Scanner stopped")
    
    async def _run_loop(self):
        while self.is_running:
            cycle_start = time.time()
            
            try:
                await self._run_cycle()
            except Exception as e:
                logger.error(f"Cycle error: {e}")
            
            cycle_time = time.time() - cycle_start
            
            self._cycle_times.append(cycle_time)
            if len(self._cycle_times) > self._max_cycle_times:
                self._cycle_times.pop(0)
            
            self.stats.avg_cycle_time = sum(self._cycle_times) / len(self._cycle_times)
            self.stats.last_cycle_time = cycle_time
            
            sleep_time = max(0, self.config.cycle_interval - cycle_time)
            if sleep_time > 0:
                await asyncio.sleep(sleep_time)
    
    async def _cleanup_loop(self):
        while self.is_running:
            try:
                expired_events = await self.events_cache.cleanup_expired()
                expired_surebets = await surebet_cache.cleanup_expired()
                stale = self.event_pool.cleanup_stale()
                
                if expired_events or expired_surebets or stale:
                    logger.debug(f"Cleaned {expired_events} events, {expired_surebets} surebets, {stale} stale")
            except:
                pass
            
            memory_manager.on_cycle()
            
            await asyncio.sleep(60)
    
    async def _stats_loop(self):
        while self.is_running:
            try:
                if self.stats.started_at:
                    self.stats.uptime_seconds = time.time() - self.stats.started_at
                
                if self.stats.total_cycles > 0:
                    self.stats.surebets_per_cycle = self.stats.total_surebets / self.stats.total_cycles
                    self.stats.events_per_cycle = self.stats.total_events / self.stats.total_cycles
            except:
                pass
            
            await asyncio.sleep(5)
    
    async def _run_cycle(self):
        self.stats.total_cycles += 1
        cycle_start = time.time()
        
        all_events = await self._fetch_all_events()
        
        if self.config.live_only:
            all_events = [e for e in all_events if e.get('is_live', False)]
        
        min_odds = self.config.min_odds
        max_odds = self.config.max_odds
        all_events = [
            e for e in all_events
            if e.get('home_odds', 0) >= min_odds
            and e.get('away_odds', 0) >= min_odds
            and e.get('home_odds', 0) <= max_odds
            and e.get('away_odds', 0) <= max_odds
        ]
        
        changed_events = self.event_pool.get_changed(all_events)
        events_to_process = changed_events if changed_events else all_events
        
        for event in events_to_process:
            key = self._get_event_key(event)
            cached = await self.events_cache.get(key)
            if cached is None:
                await self.events_cache.set(key, event, ttl=self.config.cache_ttl)
                self.perf_monitor.record_cache_miss()
            else:
                self.perf_monitor.record_cache_hit()
        
        self.events = self._group_events_by_match(all_events)
        self.stats.total_events = len(all_events)
        
        self.generosity_index.calculate_index(all_events)
        
        mirror_start = time.monotonic()
        self.mirror_detector.compute_correlation_matrix(all_events)
        mirror_time = time.monotonic() - mirror_start
        
        surebet_start = time.monotonic()
        new_surebets = self._find_surebets_optimized(all_events)
        surebet_time = time.monotonic() - surebet_start
        self.perf_monitor.record_surebet_detection(surebet_time, len(new_surebets))
        
        value_start = time.monotonic()
        self.value_bets = self.value_detector.find_value_bets(all_events)
        self.stats.total_value_bets = len(self.value_bets)
        value_time = time.monotonic() - value_start
        
        corridor_start = time.monotonic()
        self.corridors = self.corridor_finder.find_corridors(all_events)
        self.stats.total_corridors = len(self.corridors)
        corridor_time = time.monotonic() - corridor_start
        
        error_start = time.monotonic()
        self.odds_errors = self.odds_error_detector.get_errors(all_events)
        self.stats.total_odds_errors = len(self.odds_errors)
        error_time = time.monotonic() - error_start
        
        if self.odds_errors:
            logger.info(
                f"⚠️ Odds Error Detector: {len(self.odds_errors)} anomalies found "
                f"in {error_time:.2f}s"
            )
            for err in self.odds_errors[:5]:
                logger.info(
                    f"  [{err['confidence']}] {err['event_name']} | "
                    f"{err['bookmaker']} {err['selection']} @ {err['anomalous_odds']:.2f} "
                    f"(market: {err['market_average']:.2f}, dev: {err['deviation_percent']:.1f}%, "
                    f"score: {err['score']:.0f})"
                )
            
            if self._telegram_callback:
                for err in self.odds_errors[:3]:
                    if err['score'] >= 60:
                        try:
                            await self._telegram_callback({
                                'type': 'odds_error',
                                'event_name': err['event_name'],
                                'bookmaker': err['bookmaker'],
                                'selection': err['selection'],
                                'odds': err['anomalous_odds'],
                                'market_avg': err['market_average'],
                                'deviation': err['deviation_percent'],
                                'score': err['score'],
                                'action': err['action'],
                            })
                        except:
                            pass
        
        for parser_slug, parser_stat in self.stats.parser_stats.items():
            self.reliability_scorer.apply_from_parser_stats(parser_slug, parser_stat)
        
        if new_surebets:
            verified, expired = self.odds_verifier.verify_batch(new_surebets, all_events)
            self.stats.verified_count += len(verified)
            self.stats.expired_count += len(expired)
            
            if expired:
                logger.info(
                    f"🔍 Odds Verifier: {len(expired)} expired, {len(verified)} valid "
                    f"out of {len(new_surebets)} found"
                )
            
            self.freebet_surebets = self.freebet_hunter.find_freebet_surebets(verified)
            
            fresh = [sb for sb in verified if not self.seen_surebet_ids.might_have({'id': sb['id']})]
            
            if fresh:
                self.surebets = (fresh + self.surebets)[:100]
                self.stats.total_surebets = len(self.surebets)
                for sb in fresh:
                    self.seen_surebet_ids.upsert({'id': sb['id'], 'ts': time.time()})
                
                for sb in fresh:
                    await self.database.save_surebet(sb)
                    from services.analytics import analytics_engine
                    await analytics_engine.record_surebet(sb)
                    await self.surebet_history.save_surebet(sb)
                
                self._notify_subscribers(fresh)
                
                try:
                    from api.websocket import ws_manager
                    asyncio.create_task(ws_manager.send_surebets_update(self.surebets[:20]))
                    for sb in fresh:
                        asyncio.create_task(ws_manager.send_new_surebet(sb))
                except:
                    pass
                
                if self._telegram_callback:
                    for sb in fresh:
                        profit = sb.get('profit_percent', 0)
                        if profit >= self.config.min_profit:
                            await self._telegram_callback(sb)
        
        cycle_time = time.time() - cycle_start
        self.perf_monitor.record_cycle(cycle_time, len(all_events))
        self.perf_monitor.log_summary(self.stats.total_cycles)
        
        memory_manager.on_cycle()
        
        cycle_time_ms = cycle_time * 1000
        
        if self.stats.total_cycles % 10 == 0:
            cache_stats = self.events_cache.stats()
            perf = self.perf_monitor.get_throughput()
            pool_stats = self.event_pool.stats()
            mem = memory_manager.get_memory_usage()
            logger.info(
                f"📊 Cycle #{self.stats.total_cycles} | "
                f"Events: {len(all_events)} | "
                f"Surebets: {len(self.surebets)} | "
                f"Cache hit: {cache_stats.get('hit_rate', 0)}% | "
                f"Pool: {pool_stats['size']} | "
                f"Mem: {mem['current_mb']}MB | "
                f"Time: {cycle_time_ms:.0f}ms | "
                f"EPS: {perf['events_per_second']}"
            )
            
            try:
                from api.websocket import ws_manager
                asyncio.create_task(ws_manager.send_stats_update(self.get_stats()))
            except:
                pass
    
    async def _fetch_all_events(self) -> List[Dict]:
        # FIX: Run parsers in parallel with per-parser timeout
        priority_parsers = sorted(
            self.parsers,
            key=lambda p: self._get_parser_priority(p)
        )
        
        parser_timeout = 25.0
        tasks = []
        for parser in priority_parsers:
            task = asyncio.create_task(
                self._fetch_parser_events_with_timeout(parser, parser_timeout)
            )
            tasks.append((parser, task))
        
        results = await asyncio.gather(
            *[task for _, task in tasks],
            return_exceptions=True
        )
        
        all_events = []
        for (parser, _), result in zip(tasks, results):
            if isinstance(result, Exception):
                logger.error(f"Parser {parser.name} error: {result}")
                self.stats.parser_stats[parser.slug] = {'error': str(result)}
            elif isinstance(result, list):
                for event in result:
                    event['bookmaker'] = parser.slug
                all_events.extend(result)
                self.stats.parser_stats[parser.slug] = {
                    'events': len(result),
                    'requests': getattr(parser, '_request_count', 0),
                    'errors': getattr(parser, '_errors', 0)
                }
        
        return self._deduplicate_events(all_events)
    
    def _get_parser_priority(self, parser) -> int:
        priority_map = {
            'winline': 1, 'pari': 2, 'betcity': 3, 'marathon': 4,
            'zenit': 5, 'bettery': 6, 'baltbet': 7, 'tennisi': 8,
            'betm': 9, 'melbet': 10,
        }
        return priority_map.get(parser.slug, 50)
    
    async def _fetch_parser_events_with_timeout(self, parser, timeout: float) -> List[Dict]:
        try:
            return await asyncio.wait_for(
                self._fetch_parser_events(parser),
                timeout=timeout
            )
        except asyncio.TimeoutError:
            logger.warning(f"Parser {parser.name} timed out after {timeout}s")
            self.stats.parser_stats[parser.slug] = {'error': f'timeout after {timeout}s'}
            return []
    
    async def _fetch_parser_events(self, parser) -> List[Dict]:
        start = self.perf_monitor.record_parse_start(parser.slug)
        parse_start = time.monotonic()
        try:
            events = await parser.get_events()
            parse_time = time.monotonic() - parse_start
            self.perf_monitor.record_parse_end(parser.slug, start, len(events))
            logger.info(f"{parser.name}: {len(events)} events in {parse_time:.1f}s")
            return events
        except Exception as e:
            parse_time = time.monotonic() - parse_start
            logger.error(f"Parser {parser.name} failed in {parse_time:.1f}s: {e}")
            self.perf_monitor.record_parse_end(parser.slug, start, 0, had_error=True)
            return []
    
    def _detect_incremental_changes(self, events: List[Dict]) -> Optional[List[Dict]]:
        if not self._last_events_snapshot:
            self._last_events_snapshot = {
                self._get_event_key(e): hash(str(e.get('home_odds', 0)) + str(e.get('away_odds', 0)))
                for e in events if self._get_event_key(e)
            }
            return None
        
        changed = []
        current_keys = set()
        
        for event in events:
            key = self._get_event_key(event)
            if not key:
                continue
            current_keys.add(key)
            
            event_hash = hash(str(event.get('home_odds', 0)) + str(event.get('away_odds', 0)))
            if key not in self._last_events_snapshot or self._last_events_snapshot[key] != event_hash:
                changed.append(event)
                self._last_events_snapshot[key] = event_hash
        
        removed_keys = set(self._last_events_snapshot.keys()) - current_keys
        for key in removed_keys:
            del self._last_events_snapshot[key]
        
        return changed if changed else None
    
    def _deduplicate_events(self, events: List[Dict]) -> List[Dict]:
        seen = set()
        unique = []
        
        for event in events:
            key = self._get_event_key(event)
            if key and key not in seen:
                seen.add(key)
                unique.append(event)
        
        return unique
    
    def _get_event_key(self, event: Dict) -> str:
        home = event.get('home_team', '').lower().strip()
        away = event.get('away_team', '').lower().strip()
        bookmaker = event.get('bookmaker', '')
        
        if home and away and bookmaker:
            return f"{home}|{away}|{bookmaker}"
        return ""
    
    def _group_events_by_match(self, events: List[Dict]) -> Dict[str, List[Dict]]:
        grouped = defaultdict(list)
        
        for event in events:
            home, away = event_normalizer.normalize_event(
                event.get('home_team', ''),
                event.get('away_team', '')
            )
            key = f"{home}|{away}"
            grouped[key].append(event)
        
        return dict(grouped)
    
    def _find_surebets(self, events: List[Dict]) -> List[Dict]:
        if self.use_optimized and self.parallel_detector and len(events) > 500:
            return self.parallel_detector.find_surebets_parallel(events)
        return self.calculator.find_surebets(events)
    
    def _find_surebets_optimized(self, events: List[Dict]) -> List[Dict]:
        """Find surebets while skipping mirror BK pairs."""
        mirror_pairs = self.mirror_detector.get_mirror_pairs()
        skip_set = set()
        for bk_a, bk_b in mirror_pairs:
            skip_set.add((bk_a, bk_b))
            skip_set.add((bk_b, bk_a))
        
        if self.use_optimized and self.parallel_detector and len(events) > 500:
            all_surebets = self.parallel_detector.find_surebets_parallel(events)
        else:
            all_surebets = self.calculator.find_surebets(events)
        
        filtered = [
            sb for sb in all_surebets
            if not self._is_mirror_surebet(sb, skip_set)
        ]
        
        skipped = len(all_surebets) - len(filtered)
        if skipped > 0:
            logger.info(f"[MirrorDetector] Skipped {skipped} surebets from mirror pairs")
        
        return filtered
    
    def _is_mirror_surebet(self, surebet: Dict, skip_set: set) -> bool:
        """Check if a surebet involves only mirror bookmakers."""
        bks = surebet.get("bookmakers", [])
        if len(bks) < 2:
            return False
        for i in range(len(bks)):
            for j in range(i + 1, len(bks)):
                if (bks[i], bks[j]) not in skip_set:
                    return False
        return True
    
    def subscribe(self, callback: Callable):
        self._subscribers.append(callback)
    
    def unsubscribe(self, callback: Callable):
        if callback in self._subscribers:
            self._subscribers.remove(callback)
    
    def set_telegram_callback(self, callback: Callable):
        self._telegram_callback = callback
    
    def _notify_subscribers(self, surebets: List[Dict]):
        for callback in self._subscribers:
            try:
                callback(surebets)
            except Exception as e:
                logger.error(f"Subscriber callback error: {e}")
    
    def get_events(self) -> List[Dict]:
        result = []
        for events_list in self.events.values():
            result.extend(events_list)
        return result
    
    def get_surebets(self, min_profit: float = 0.0, sport: Optional[str] = None) -> List[Dict]:
        filtered = self.surebets
        
        if min_profit > 0:
            filtered = [sb for sb in filtered if sb.get('profit_percent', 0) >= min_profit]
        
        if sport:
            filtered = [sb for sb in filtered if sb.get('sport') == sport]
        
        return filtered
    
    def get_top_surebets(self, limit: int = 10) -> List[Dict]:
        return self.surebets[:limit]
    
    def get_value_bets(self, min_edge: float = 2.0, sport: Optional[str] = None) -> List[Dict]:
        return self.value_detector.find_value_bets(
            events=self.get_events(),
            min_edge=min_edge,
            sport=sport,
        )
    
    def get_corridors(self, min_ev: float = 1.0, sport: Optional[str] = None) -> List[Dict]:
        return self.corridor_finder.find_corridors(
            events=self.get_events(),
            min_ev=min_ev,
            sport=sport,
        )
    
    def get_stats(self) -> Dict:
        uptime = time.time() - self.stats.started_at if self.stats.started_at else 0
        uptime_str = f"{int(uptime // 3600)}h {int((uptime % 3600) // 60)}m" if uptime > 0 else "0m"
        
        return {
            'is_running': self.is_running,
            'uptime': uptime_str,
            'uptime_seconds': uptime,
            'total_cycles': self.stats.total_cycles,
            'total_events': self.stats.total_events,
            'total_surebets': len(self.surebets),
            'total_value_bets': self.stats.total_value_bets,
            'total_corridors': self.stats.total_corridors,
            'total_odds_errors': self.stats.total_odds_errors,
            'verified_count': self.stats.verified_count,
            'expired_count': self.stats.expired_count,
            'last_cycle_time_ms': round(self.stats.last_cycle_time * 1000, 2) if self.stats.last_cycle_time else 0,
            'avg_cycle_time_ms': round(self.stats.avg_cycle_time * 1000, 2),
            'surebets_per_cycle': round(self.stats.surebets_per_cycle, 2),
            'events_per_cycle': round(self.stats.events_per_cycle, 1),
            'cache_stats': self.events_cache.stats(),
            'parsers': self.stats.parser_stats,
            'sources': list(self.config.enabled_sources),
            'performance': self.perf_monitor.get_full_report(),
            'value_detector': self.value_detector.get_stats(),
            'corridor_finder': self.corridor_finder.get_stats(),
            'reliability': self.reliability_scorer.get_summary(),
            'generosity': self.generosity_index.get_summary(),
            'odds_verifier': self.odds_verifier.get_stats(),
            'mirror_detector': self.mirror_detector.get_summary(),
            'momentum_scanner': self.momentum_scanner.get_stats(),
            'freebet_hunter': {
                'available_freebets': len(self.freebet_hunter.get_available_freebets()),
                'freebet_surebets': len(self.freebet_surebets),
                'best_freebet': self.freebet_hunter.get_best_freebet_strategy(self.surebets),
            },
            'config': self.get_config(),
        }
    
    def get_config(self) -> Dict:
        return {
            'min_profit': self.config.min_profit,
            'cycle_interval': self.config.cycle_interval,
            'max_events_per_source': self.config.max_events_per_source,
            'cache_ttl': self.config.cache_ttl,
            'enabled_sources': list(self.config.enabled_sources),
            'live_only': self.config.live_only,
            'prematch_enabled': self.config.prematch_enabled,
            'cyber_enabled': self.config.cyber_enabled,
            'min_odds': self.config.min_odds,
            'max_odds': self.config.max_odds,
        }
    
    def update_config(self, **kwargs) -> Dict:
        for key, value in kwargs.items():
            if hasattr(self.config, key):
                setattr(self.config, key, value)
        
        if 'enabled_sources' in kwargs:
            self._update_parsers()
        
        return self.get_config()
    
    def _update_parsers(self):
        current_slugs = {p.slug for p in self.parsers}
        target_slugs = self.config.enabled_sources
        
        for slug in target_slugs - current_slugs:
            self._add_parser(slug)
        
        for slug in current_slugs - target_slugs:
            self._remove_parser(slug)
    
    def _add_parser(self, slug: str):
        playwright_parsers = {
            'winline': ('scanner.parsers.winline_playwright', 'WinlinePlaywrightParser'),
            'pari': ('scanner.parsers.pari_playwright', 'PariPlaywrightParser'),
            'marathon': ('scanner.parsers.marathon_playwright', 'MarathonPlaywrightParser'),
            'zenit': ('scanner.parsers.zenit_playwright', 'ZenitPlaywrightParser'),
            'bettery': ('scanner.parsers.bettery_playwright', 'BetteryPlaywrightParser'),
            'baltbet': ('scanner.parsers.baltbet_playwright', 'BaltbetRegexParser'),
            'betboom': ('scanner.parsers.betboom_playwright', 'BetBoomPlaywrightParser'),
            'fonbet': ('scanner.parsers.fonbet_playwright', 'FonbetPlaywrightParser'),
            'ligastavok': ('scanner.parsers.ligastavok_api', 'LigaStavokPlaywrightParser'),
            'sportbet': ('scanner.parsers.sportbet_playwright', 'SportbetPlaywrightParser'),
            '24bet': ('scanner.parsers._24bet_playwright', '_24betPlaywrightParser'),
            'betboo': ('scanner.parsers.betboo_playwright', 'BetBooPlaywrightParser'),
        }
        
        if slug in playwright_parsers:
            module_name, class_name = playwright_parsers[slug]
            try:
                module = __import__(module_name, fromlist=[class_name])
                cls = getattr(module, class_name)
                self.parsers.append(cls())
                logger.info(f"Added parser: {slug}")
            except Exception as e:
                logger.warning(f"Failed to add parser {slug}: {e}")
    
    def _remove_parser(self, slug: str):
        self.parsers = [p for p in self.parsers if p.slug != slug]
        logger.info(f"Removed parser: {slug}")
    
    def get_bookmaker_stats(self) -> Dict:
        stats = defaultdict(lambda: {'events': 0, 'errors': 0})
        
        for events_list in self.events.values():
            for event in events_list:
                bk = event.get('bookmaker', 'unknown')
                stats[bk]['events'] += 1
        
        for parser_slug, parser_stat in self.stats.parser_stats.items():
            stats[parser_slug].update(parser_stat)
        
        return dict(stats)
