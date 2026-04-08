# scanner/advanced_runner.py
import asyncio
from datetime import datetime
from typing import List, Dict, Set, Optional, Callable
from dataclasses import dataclass, field
import logging
import json
from collections import defaultdict

from core.surebet_calculator import SurebetCalculator
from scanner.parsers import ALL_PARSERS
from services.database import Database

logger = logging.getLogger(__name__)


@dataclass
class ScannerConfig:
    min_profit: float = 0.5
    cycle_interval: int = 5
    max_events_per_source: int = 100
    enabled_sources: Set[str] = field(default_factory=lambda: {
        'winline', 'olimp', 'pari', 'marathon', 'betboom', 'fonbet',
        '1xstavka', 'leon', 'betcity', 'pinup', 'zenit', 'olimpbet'
    })


class AdvancedScannerRunner:
    def __init__(self, database: Database, config: ScannerConfig = None):
        self.database = database
        self.config = config or ScannerConfig()
        
        self.calculator = SurebetCalculator(min_profit=self.config.min_profit)
        
        self.parsers = self._init_parsers()
        
        self.events: List[Dict] = []
        self.surebets: List[Dict] = []
        self.seen_events: Set[str] = set()
        
        self.is_running = False
        self.cycle_count = 0
        self.last_cycle_time: Optional[datetime] = None
        self.last_surebets_time: Optional[datetime] = None
        
        self.stats = {
            'total_cycles': 0,
            'total_events': 0,
            'total_surebets': 0,
            'sources': defaultdict(lambda: {'events': 0, 'errors': 0})
        }
        
        self.websocket_connections: Set[Callable] = set()
        self.telegram_callback: Optional[Callable] = None
    
    def _init_parsers(self):
        parsers = []
        for parser_class in ALL_PARSERS:
            if parser_class.slug in self.config.enabled_sources:
                parsers.append(parser_class())
        logger.info(f"Initialized {len(parsers)} parsers")
        return parsers
    
    async def start(self):
        if self.is_running:
            return
        
        self.is_running = True
        logger.info(f"Advanced Scanner started with {len(self.parsers)} parsers")
        
        asyncio.create_task(self._run_loop())
    
    async def stop(self):
        self.is_running = False
        
        for parser in self.parsers:
            if hasattr(parser, 'close'):
                try:
                    await parser.close()
                except:
                    pass
        
        logger.info("Scanner stopped")
    
    async def _run_loop(self):
        while self.is_running:
            try:
                await self.run_cycle()
            except Exception as e:
                logger.error(f"Cycle error: {e}")
            
            await asyncio.sleep(self.config.cycle_interval)
    
    async def run_cycle(self):
        self.cycle_count += 1
        start_time = datetime.utcnow()
        
        all_events = []
        source_stats = defaultdict(lambda: {'events': 0, 'errors': 0})
        
        tasks = [parser.get_events() for parser in self.parsers]
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        for parser, result in zip(self.parsers, results):
            if isinstance(result, Exception):
                logger.error(f"Parser {parser.name} error: {result}")
                source_stats[parser.slug]['errors'] += 1
            elif isinstance(result, list):
                for event in result:
                    event['bookmaker'] = parser.slug
                    all_events.append(event)
                source_stats[parser.slug]['events'] = len(result)
        
        self.events = self._deduplicate_events(all_events)
        self.stats['total_events'] = len(self.events)
        self.stats['sources'] = dict(source_stats)
        
        new_surebets = self.calculator.find_surebets(self.events)
        
        if new_surebets:
            fresh_surebets = self._filter_new_surebets(new_surebets)
            
            if fresh_surebets:
                self.surebets = self._merge_surebets(new_surebets)
                self.last_surebets_time = datetime.utcnow()
                
                for surebet in fresh_surebets:
                    await self.database.save_surebet(surebet)
                
                self._notify_websocket(fresh_surebets)
                
                if self.telegram_callback:
                    for sb in fresh_surebets:
                        if sb.get('profit_percent', 0) >= 5.0:
                            await self.telegram_callback(sb)
        
        self.last_cycle_time = datetime.utcnow()
        self.stats['total_cycles'] += 1
        
        duration_ms = (datetime.utcnow() - start_time).total_seconds() * 1000
        
        if self.cycle_count % 10 == 0:
            logger.info(
                f"Cycle #{self.cycle_count} | "
                f"Events: {len(self.events)} | "
                f"Surebets: {len(self.surebets)} | "
                f"Duration: {duration_ms:.0f}ms"
            )
    
    def _deduplicate_events(self, events: List[Dict]) -> List[Dict]:
        seen = set()
        unique = []
        
        for event in events:
            key = self._event_key(event)
            if key and key not in seen:
                seen.add(key)
                unique.append(event)
        
        return unique
    
    def _event_key(self, event: Dict) -> str:
        home = event.get('home_team', '').lower().strip()
        away = event.get('away_team', '').lower().strip()
        bookmaker = event.get('bookmaker', '')
        
        if home and away and bookmaker:
            return f"{home}|{away}|{bookmaker}"
        return ""
    
    def _filter_new_surebets(self, surebets: List[Dict]) -> List[Dict]:
        seen_keys = set(s['id'] for s in self.surebets)
        return [s for s in surebets if s['id'] not in seen_keys]
    
    def _merge_surebets(self, new: List[Dict]) -> List[Dict]:
        existing = {s['id']: s for s in self.surebets}
        
        for sb in new:
            existing[sb['id']] = sb
        
        sorted_sb = sorted(
            existing.values(),
            key=lambda x: x.get('profit_percent', 0),
            reverse=True
        )
        
        return sorted_sb[:100]
    
    def _notify_websocket(self, surebets: List[Dict]):
        for callback in self.websocket_connections:
            try:
                callback(surebets)
            except Exception as e:
                logger.error(f"WebSocket notify error: {e}")
    
    def subscribe_websocket(self, callback: Callable):
        self.websocket_connections.add(callback)
    
    def unsubscribe_websocket(self, callback: Callable):
        self.websocket_connections.discard(callback)
    
    def set_telegram_callback(self, callback: Callable):
        self.telegram_callback = callback
    
    def get_events(self) -> List[Dict]:
        return self.events.copy()
    
    def get_surebets(self) -> List[Dict]:
        return self.surebets.copy()
    
    def get_top_surebets(self, limit: int = 10) -> List[Dict]:
        return self.surebets[:limit]
    
    def get_bookmaker_stats(self) -> Dict:
        return dict(self.stats['sources'])
    
    def get_stats(self) -> Dict:
        return {
            **self.stats,
            'is_running': self.is_running,
            'cycle_count': self.cycle_count,
            'events_count': len(self.events),
            'surebets_count': len(self.surebets),
            'last_cycle': self.last_cycle_time.isoformat() if self.last_cycle_time else None,
            'sources': dict(self.stats['sources']),
            'parsers_count': len(self.parsers)
        }
    
    async def search_surebet(self, query: str) -> List[Dict]:
        query_lower = query.lower()
        return [
            sb for sb in self.surebets
            if query_lower in sb.get('event_name', '').lower()
        ]
