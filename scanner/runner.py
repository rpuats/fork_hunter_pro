# scanner/runner.py
import asyncio
from datetime import datetime
from typing import List, Dict
import structlog

from core.surebet_calculator import SurebetCalculator
from scanner.parsers.base import BaseParser
from scanner.parsers.winline_parser import WinlineParser
from scanner.parsers.olimp_parser import OlimpParser
from scanner.parsers.pari_parser import PariParser
from scanner.parsers.marathon_parser import MarathonParser
from scanner.parsers.betboom_parser import BetBoomParser

logger = structlog.get_logger()


class ScannerRunner:
    def __init__(self, database=None):
        self.database = database
        self.parsers: List[BaseParser] = [
            WinlineParser(),
            OlimpParser(),
            PariParser(),
            MarathonParser(),
            BetBoomParser(),
        ]
        self.calculator = SurebetCalculator(min_profit=0.5)
        self.events: List[Dict] = []
        self.surebets: List[Dict] = []
        self.is_running = False
        self.last_scan_time = None
        self.cycle_count = 0
        
    async def start(self):
        self.is_running = True
        logger.info("Scanner started")
        
        while self.is_running:
            await self.run_cycle()
            await asyncio.sleep(5)
    
    async def stop(self):
        self.is_running = False
        logger.info("Scanner stopped")
    
    async def run_cycle(self):
        self.cycle_count += 1
        start_time = datetime.utcnow()
        
        all_events = []
        for parser in self.parsers:
            try:
                events = await parser.get_events()
                all_events.extend(events)
            except Exception as e:
                logger.error(f"Parser {parser.name} failed", error=str(e))
        
        self.events = all_events
        self.surebets = self.calculator.find_surebets(all_events)
        self.last_scan_time = datetime.utcnow()
        
        logger.info(
            "Cycle completed",
            cycle=self.cycle_count,
            events=len(all_events),
            surebets=len(self.surebets),
            duration=(datetime.utcnow() - start_time).total_seconds()
        )
    
    def get_events(self) -> List[Dict]:
        return self.events.copy()
    
    def get_surebets(self) -> List[Dict]:
        return self.surebets.copy()
    
    def get_bookmaker_stats(self) -> Dict:
        stats = {}
        for event in self.events:
            bk = event.get('bookmaker')
            if bk not in stats:
                stats[bk] = {'events': 0}
            stats[bk]['events'] += 1
        return stats
