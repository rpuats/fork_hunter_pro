# scanner/live_scanner.py
"""
Live Scanner - Real-time odds monitoring
WebSocket + Adaptive Polling for live events
"""
import asyncio
import time
import logging
from typing import Dict, List, Optional, Callable
from dataclasses import dataclass
from collections import defaultdict

logger = logging.getLogger(__name__)


@dataclass
class LiveConfig:
    """Configuration for live scanning"""
    websocket_enabled: bool = True
    polling_interval_live: float = 1.0  # seconds
    polling_interval_prematch: float = 5.0  # seconds
    max_latency_ms: float = 5000.0  # max acceptable delay
    flash_alert_threshold: float = 3.0  # % profit for flash alert


class LiveScanner:
    """
    High-speed live scanner with:
    - WebSocket subscriptions (when available)
    - Adaptive polling for different bookmakers
    - Flash alerts for hot opportunities
    - Latency monitoring
    """
    
    def __init__(self, config: Optional[LiveConfig] = None):
        self.config = config or LiveConfig()
        self.websocket_connections: Dict[str, asyncio.Task] = {}
        self.last_odds: Dict[str, Dict] = {}  # event_id -> odds snapshot
        self.odds_changes: Dict[str, List[float]] = defaultdict(list)
        self.latency_history: List[float] = []
        self.flash_callback: Optional[Callable] = None
        self._running = False
    
    async def start(self):
        """Start live scanning"""
        self._running = True
        logger.info("Live Scanner started")
        
        # Start WebSocket connections for supported bookmakers
        if self.config.websocket_enabled:
            await self._start_websocket_scanners()
    
    async def stop(self):
        """Stop live scanning"""
        self._running = False
        
        # Cancel all WebSocket tasks
        for task in self.websocket_connections.values():
            task.cancel()
        
        logger.info("Live Scanner stopped")
    
    async def _start_websocket_scanners(self):
        """Start WebSocket connections for live data"""
        # These bookmakers support WebSocket
        websocket_bks = {
            'winline': self._ws_winline,
            'betboom': self._ws_betboom,
        }
        
        for bk, handler in websocket_bks.items():
            task = asyncio.create_task(handler())
            self.websocket_connections[bk] = task
    
    async def _ws_winline(self):
        """WebSocket handler for Winline"""
        import aiohttp
        
        ws_url = "wss://winline.ru/ws/live"
        headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        }
        
        while self._running:
            try:
                async with aiohttp.ClientSession() as session:
                    async with session.ws_connect(ws_url, headers=headers) as ws:
                        logger.info("Winline WebSocket connected")
                        
                        async for msg in ws:
                            if not self._running:
                                break
                            
                            if msg.type == aiohttp.WSMsgType.TEXT:
                                start = time.time()
                                data = msg.json()
                                await self._process_live_data('winline', data)
                                latency = (time.time() - start) * 1000
                                self._record_latency(latency)
                                
                            elif msg.type == aiohttp.WSMsgType.ERROR:
                                logger.error(f"Winline WS error: {msg.data}")
                                break
                
            except Exception as e:
                logger.error(f"Winline WS reconnecting: {e}")
                await asyncio.sleep(5)
    
    async def _ws_betboom(self):
        """WebSocket handler for BetBoom"""
        import aiohttp
        
        ws_url = "wss://betboom.ru/ws"
        
        while self._running:
            try:
                async with aiohttp.ClientSession() as session:
                    async with session.ws_connect(ws_url) as ws:
                        logger.info("BetBoom WebSocket connected")
                        
                        async for msg in ws:
                            if not self._running:
                                break
                            
                            if msg.type == aiohttp.WSMsgType.TEXT:
                                start = time.time()
                                data = msg.json()
                                await self._process_live_data('betboom', data)
                                latency = (time.time() - start) * 1000
                                self._record_latency(latency)
                
            except Exception as e:
                logger.error(f"BetBoom WS reconnecting: {e}")
                await asyncio.sleep(5)
    
    async def _process_live_data(self, bookmaker: str, data: Dict):
        """Process incoming live data"""
        try:
            events = data.get('events', [])
            
            for event in events:
                event_id = f"{bookmaker}_{event.get('id', '')}"
                
                # Check for odds changes
                new_odds = {
                    'home': event.get('k1', 0),
                    'draw': event.get('kx', 0),
                    'away': event.get('k2', 0),
                }
                
                if event_id in self.last_odds:
                    old_odds = self.last_odds[event_id]
                    
                    # Calculate odds movement
                    for key in ['home', 'draw', 'away']:
                        if old_odds.get(key) != new_odds.get(key):
                            change_pct = abs(new_odds[key] - old_odds[key]) / old_odds.get(key, 1) * 100
                            self.odds_changes[event_id].append(change_pct)
                            
                            # Keep only last 10 changes
                            if len(self.odds_changes[event_id]) > 10:
                                self.odds_changes[event_id] = self.odds_changes[event_id][-10:]
                
                self.last_odds[event_id] = new_odds
                
        except Exception as e:
            logger.error(f"Error processing {bookmaker} live data: {e}")
    
    async def poll_bookmaker(self, bookmaker: str, parser) -> List[Dict]:
        """
        Poll a bookmaker for live data.
        Returns list of changed events since last poll.
        """
        try:
            events = await parser.get_events()
            
            changed_events = []
            for event in events:
                event_id = f"{bookmaker}_{event.get('id', '')}"
                
                if event_id not in self.last_odds:
                    self.last_odds[event_id] = {
                        'home': event.get('home_odds', 0),
                        'away': event.get('away_odds', 0),
                        'draw': event.get('draw_odds', 0),
                    }
                    changed_events.append(event)
                else:
                    old = self.last_odds[event_id]
                    new_home = event.get('home_odds', 0)
                    new_away = event.get('away_odds', 0)
                    
                    if abs(old['home'] - new_home) > 0.01 or abs(old['away'] - new_away) > 0.01:
                        self.last_odds[event_id] = {
                            'home': new_home,
                            'away': new_away,
                            'draw': event.get('draw_odds', 0),
                        }
                        changed_events.append(event)
            
            return changed_events
            
        except Exception as e:
            logger.error(f"Error polling {bookmaker}: {e}")
            return []
    
    def _record_latency(self, latency_ms: float):
        """Record latency for monitoring"""
        self.latency_history.append(latency_ms)
        
        # Keep last 100 measurements
        if len(self.latency_history) > 100:
            self.latency_history = self.latency_history[-100:]
        
        # Check if latency is acceptable
        if latency_ms > self.config.max_latency_ms:
            logger.warning(f"High latency detected: {latency_ms:.0f}ms")
    
    def get_stats(self) -> Dict:
        """Get live scanner statistics"""
        avg_latency = sum(self.latency_history) / len(self.latency_history) if self.latency_history else 0
        
        return {
            'running': self._running,
            'websocket_connections': len(self.websocket_connections),
            'tracked_events': len(self.last_odds),
            'avg_latency_ms': round(avg_latency, 2),
            'max_latency_ms': max(self.latency_history) if self.latency_history else 0,
            'ws_enabled': self.config.websocket_enabled,
        }
    
    def is_odds_stable(self, event_id: str, threshold: float = 0.5) -> bool:
        """Check if odds have been stable"""
        changes = self.odds_changes.get(event_id, [])
        
        if len(changes) < 3:
            return True  # Not enough data
        
        # Check if recent changes are small
        recent = changes[-3:]
        return all(c < threshold for c in recent)


# Global instance
live_scanner = LiveScanner()
