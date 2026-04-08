# core/momentum_scanner.py
"""
Live Momentum Scanner (Идея #10)

Trigger-based scanner that activates during match events (goals, cards, penalties).
During these 5-15 second windows, bookmakers update odds at different speeds,
creating ultra-profitable surebets (5-20% vs normal 0.5-3%).

Architecture:
  ┌─────────────────────────────────────────────────────────────┐
  │                    GhostScanner Engine                       │
  │  ┌─────────────┐    ┌──────────────────┐                    │
  │  │ Normal Scan │    │ Momentum Scanner │◄── WebSocket feeds │
  │  │  (3s cycle) │    │  (trigger-based) │                    │
  │  └─────────────┘    └────────┬─────────┘                    │
  │                              │                               │
  │                    ┌─────────▼─────────┐                    │
  │                    │  Trigger Router   │                    │
  │                    │ (goal/card/pen)   │                    │
  │                    └─────────┬─────────┘                    │
  │                              │                               │
  │                    ┌─────────▼─────────┐                    │
  │                    │ Rapid Odds Scan   │                    │
  │                    │  (500ms intervals)│                    │
  │                    └─────────┬─────────┘                    │
  │                              │                               │
  │                    ┌─────────▼─────────┐                    │
  │                    │ Momentum Surebet  │                    │
  │                    │   Detector        │                    │
  │                    └─────────┬─────────┘                    │
  │                              │                               │
  │                    ┌─────────▼─────────┐                    │
  │                    │  Priority Notify  │                    │
  │                    │ (Telegram + WS)   │                    │
  │                    └───────────────────┘                    │
  └─────────────────────────────────────────────────────────────┘
"""
import asyncio
import time
import hashlib
import logging
from enum import Enum
from dataclasses import dataclass, field
from typing import List, Dict, Optional, Callable, Set, Tuple
from datetime import datetime
from collections import defaultdict, deque

logger = logging.getLogger(__name__)


# ───────────────────────────────────────────────────────────────
# Trigger Event Types
# ───────────────────────────────────────────────────────────────

class MomentumTriggerType(Enum):
    """Types of events that trigger momentum scanning."""
    GOAL = "goal"
    RED_CARD = "red_card"
    YELLOW_CARD = "yellow_card"
    PENALTY = "penalty"
    OVERTIME = "overtime"
    PERIOD_END = "period_end"
    MATCH_START = "match_start"
    ODDS_SPIKE = "odds_spike"  # Detected from odds movement patterns


@dataclass
class MomentumTrigger:
    """Represents a trigger event that activates momentum scanning."""
    trigger_type: MomentumTriggerType
    match_key: str  # "home_team|away_team"
    sport: str = "football"
    timestamp: float = field(default_factory=time.time)
    metadata: Dict = field(default_factory=dict)
    source: str = "live_feed"  # Source of the trigger (winline_ws, external_api, etc.)
    confidence: float = 1.0  # 0.0-1.0 confidence in trigger accuracy

    @property
    def id(self) -> str:
        raw = f"{self.trigger_type.value}|{self.match_key}|{self.timestamp}"
        return hashlib.md5(raw.encode()).hexdigest()[:12]

    @property
    def window_duration(self) -> float:
        """How long to scan after this trigger (seconds)."""
        durations = {
            MomentumTriggerType.GOAL: 15.0,
            MomentumTriggerType.RED_CARD: 12.0,
            MomentumTriggerType.YELLOW_CARD: 8.0,
            MomentumTriggerType.PENALTY: 10.0,
            MomentumTriggerType.OVERTIME: 10.0,
            MomentumTriggerType.PERIOD_END: 5.0,
            MomentumTriggerType.MATCH_START: 8.0,
            MomentumTriggerType.ODDS_SPIKE: 6.0,
        }
        return durations.get(self.trigger_type, 10.0)

    @property
    def priority(self) -> int:
        """Higher priority = more urgent scanning."""
        priorities = {
            MomentumTriggerType.GOAL: 10,
            MomentumTriggerType.RED_CARD: 9,
            MomentumTriggerType.PENALTY: 8,
            MomentumTriggerType.OVERTIME: 7,
            MomentumTriggerType.MATCH_START: 6,
            MomentumTriggerType.ODDS_SPIKE: 5,
            MomentumTriggerType.PERIOD_END: 3,
            MomentumTriggerType.YELLOW_CARD: 2,
        }
        return priorities.get(self.trigger_type, 1)


# ───────────────────────────────────────────────────────────────
# Momentum Window Manager
# ───────────────────────────────────────────────────────────────

class MomentumWindow:
    """
    Manages a single momentum scanning window.
    Activated when a trigger fires, runs rapid scans until window closes.
    """

    def __init__(
        self,
        trigger: MomentumTrigger,
        scan_interval: float = 0.5,  # 500ms between scans during momentum
        min_profit: float = 5.0,  # Higher threshold for momentum surebets
    ):
        self.trigger = trigger
        self.scan_interval = scan_interval
        self.min_profit = min_profit

        self.started_at: float = time.time()
        self.expires_at: float = self.started_at + trigger.window_duration
        self.is_active: bool = True
        self.scan_count: int = 0
        self.surebets_found: int = 0
        self.odds_snapshots: deque = deque(maxlen=100)

    @property
    def time_remaining(self) -> float:
        return max(0, self.expires_at - time.time())

    @property
    def utilization(self) -> float:
        elapsed = time.time() - self.started_at
        return min(1.0, elapsed / self.trigger.window_duration)

    def record_snapshot(self, odds_data: Dict):
        """Record an odds snapshot during the momentum window."""
        self.odds_snapshots.append({
            'timestamp': time.time(),
            'data': odds_data,
            'scan_number': self.scan_count,
        })

    def should_scan(self) -> bool:
        """Check if we should run another scan."""
        return self.is_active and self.time_remaining > 0

    def close(self):
        """Close the momentum window."""
        self.is_active = False
        logger.info(
            f"🔒 Momentum window closed: {self.trigger.trigger_type.value} "
            f"on {self.trigger.match_key} | "
            f"Scans: {self.scan_count} | Surebets: {self.surebets_found}"
        )


# ───────────────────────────────────────────────────────────────
# Rapid Odds Tracker
# ───────────────────────────────────────────────────────────────

class RapidOddsTracker:
    """
    Tracks odds changes at high frequency during momentum windows.
    Detects discrepancies between bookmakers in real-time.
    """

    def __init__(self, max_history: int = 500):
        self._odds_history: Dict[str, deque] = defaultdict(
            lambda: deque(maxlen=max_history)
        )
        self._last_odds: Dict[str, Dict] = {}
        self._change_count: Dict[str, int] = defaultdict(int)

    def update(self, event_key: str, odds: Dict):
        """Record new odds for an event."""
        snapshot = {
            'timestamp': time.time(),
            'home_odds': odds.get('home_odds', 0),
            'away_odds': odds.get('away_odds', 0),
            'draw_odds': odds.get('draw_odds'),
            'bookmaker': odds.get('bookmaker', ''),
        }
        self._odds_history[event_key].append(snapshot)
        self._last_odds[event_key] = snapshot

        if len(self._odds_history[event_key]) > 1:
            self._change_count[event_key] += 1

    def get_odds_velocity(self, event_key: str, window_seconds: float = 5.0) -> float:
        """Calculate how fast odds are changing (changes per second)."""
        history = self._odds_history.get(event_key, [])
        if len(history) < 2:
            return 0.0

        now = time.time()
        recent = [s for s in history if now - s['timestamp'] <= window_seconds]
        if len(recent) < 2:
            return 0.0

        time_span = recent[-1]['timestamp'] - recent[0]['timestamp']
        if time_span <= 0:
            return 0.0

        return len(recent) / time_span

    def get_best_odds(self, match_key: str) -> Dict[str, Optional[Dict]]:
        """Get the best current odds for each outcome across all bookmakers."""
        best = {
            'home': {'odds': 0.0, 'event': None},
            'away': {'odds': 0.0, 'event': None},
            'draw': {'odds': 0.0, 'event': None},
        }

        for event_key, last in self._last_odds.items():
            if match_key.lower() not in event_key.lower():
                continue

            home = last.get('home_odds', 0)
            away = last.get('away_odds', 0)
            draw = last.get('draw_odds') or 0

            if home > best['home']['odds']:
                best['home'] = {'odds': home, 'event': last}
            if away > best['away']['odds']:
                best['away'] = {'odds': away, 'event': last}
            if draw > best['draw']['odds']:
                best['draw'] = {'odds': draw, 'event': last}

        return best

    def detect_spike(self, event_key: str, threshold: float = 0.15) -> bool:
        """Detect if odds changed by more than threshold% in last snapshot."""
        history = self._odds_history.get(event_key, [])
        if len(history) < 2:
            return False

        prev = history[-2]
        curr = history[-1]

        for key in ['home_odds', 'away_odds', 'draw_odds']:
            prev_val = prev.get(key, 0) or 0
            curr_val = curr.get(key, 0) or 0
            if prev_val > 0:
                change = abs(curr_val - prev_val) / prev_val
                if change > threshold:
                    return True

        return False


# ───────────────────────────────────────────────────────────────
# Live Feed Connector (WebSocket)
# ───────────────────────────────────────────────────────────────

class LiveFeedConnector:
    """
    WebSocket connection to live score feeds for real-time event detection.
    Integrates with existing Winline WebSocket infrastructure.
    """

    def __init__(self):
        self._connected: bool = False
        self._callbacks: List[Callable] = []
        self._ws_connections: Dict[str, any] = {}
        self._reconnect_attempts: int = 0
        self._max_reconnect_attempts: int = 5

    async def connect(self):
        """Establish WebSocket connections to live feeds."""
        # TODO: Implement WebSocket connection to:
        # 1. Winline live feed (existing infrastructure)
        # 2. External live-score API (e.g., Sportmonks, API-Football)
        # 3. Custom odds movement detection from parser streams
        logger.info("📡 LiveFeedConnector: Connecting to live feeds...")
        self._connected = True

    async def disconnect(self):
        """Close all WebSocket connections."""
        for ws in self._ws_connections.values():
            try:
                await ws.close()
            except Exception:
                pass
        self._ws_connections.clear()
        self._connected = False
        logger.info("📡 LiveFeedConnector: Disconnected")

    def on_trigger(self, callback: Callable):
        """Register callback for trigger events."""
        self._callbacks.append(callback)

    async def _emit_trigger(self, trigger: MomentumTrigger):
        """Emit a trigger event to all registered callbacks."""
        for callback in self._callbacks:
            try:
                if asyncio.iscoroutinefunction(callback):
                    await callback(trigger)
                else:
                    callback(trigger)
            except Exception as e:
                logger.error(f"Trigger callback error: {e}")

    # ─── Feed Parsers (to be implemented) ───

    async def _parse_winline_ws(self, message: dict) -> Optional[MomentumTrigger]:
        """Parse Winline WebSocket message for trigger events."""
        # TODO: Implement Winline WebSocket message parsing
        # Expected message format:
        # {
        #   "type": "event",
        #   "match_id": "...",
        #   "event_type": "goal|card|penalty",
        #   "team": "home|away",
        #   "minute": 45,
        #   ...
        # }
        pass

    async def _parse_external_feed(self, message: dict) -> Optional[MomentumTrigger]:
        """Parse external live-score API message."""
        # TODO: Implement external feed parsing
        pass

    async def _detect_odds_spike(self, event_key: str, old_odds: Dict, new_odds: Dict) -> Optional[MomentumTrigger]:
        """Detect momentum trigger from odds movement patterns."""
        # TODO: Implement odds spike detection
        # Heuristic: if odds change > 15% in < 2 seconds, likely a match event
        pass


# ───────────────────────────────────────────────────────────────
# Momentum Surebet Detector
# ───────────────────────────────────────────────────────────────

class MomentumSurebetDetector:
    """
    Specialized surebet detector for momentum windows.
    Higher profit thresholds, faster detection, priority notification.
    """

    def __init__(
        self,
        min_profit: float = 5.0,  # Much higher than normal scanner
        max_detection_time: float = 2.0,  # Must complete within 2 seconds
    ):
        self.min_profit = min_profit / 100
        self.max_detection_time = max_detection_time
        self._detection_count: int = 0
        self._success_count: int = 0

    def detect(
        self,
        events: List[Dict],
        trigger: MomentumTrigger,
    ) -> List[Dict]:
        """
        Rapid surebet detection during momentum window.
        Optimized for speed over completeness.
        """
        start = time.monotonic()

        # Group events by match (same as normal scanner)
        grouped = self._group_events(events)

        surebets = []
        for match_key, match_events in grouped.items():
            if time.monotonic() - start > self.max_detection_time:
                logger.warning("⏱️ Momentum detection timeout")
                break

            # Only check matches related to the trigger
            if trigger.match_key.lower() not in match_key.lower():
                continue

            # Fast 2-way detection
            sb = self._detect_2way_fast(match_events, match_key)
            if sb:
                sb['trigger_type'] = trigger.trigger_type.value
                sb['momentum_window'] = trigger.window_duration
                sb['time_to_expire'] = trigger.window_duration
                surebets.append(sb)

            # Fast 3-way detection (only if time permits)
            if time.monotonic() - start < self.max_detection_time * 0.7:
                sb3 = self._detect_3way_fast(match_events, match_key)
                if sb3:
                    sb3['trigger_type'] = trigger.trigger_type.value
                    sb3['momentum_window'] = trigger.window_duration
                    sb3['time_to_expire'] = trigger.window_duration
                    surebets.append(sb3)

        elapsed = time.monotonic() - start
        self._detection_count += 1
        self._success_count += len(surebets)

        if surebets:
            logger.info(
                f"⚡ MOMENTUM SUREBET DETECTED! "
                f"Trigger: {trigger.trigger_type.value} | "
                f"Found: {len(surebets)} | "
                f"Time: {elapsed*1000:.0f}ms"
            )

        return surebets

    def _group_events(self, events: List[Dict]) -> Dict[str, List[Dict]]:
        """Fast event grouping."""
        grouped = defaultdict(list)
        for e in events:
            home = e.get('home_team', '').lower().strip()
            away = e.get('away_team', '').lower().strip()
            if home and away:
                key = f"{home}|{away}"
                grouped[key].append(e)
        return dict(grouped)

    def _detect_2way_fast(self, events: List[Dict], match_key: str) -> Optional[Dict]:
        """Ultra-fast 2-way surebet detection."""
        best_home = {'odds': 0, 'event': None}
        best_away = {'odds': 0, 'event': None}

        for e in events:
            h = e.get('home_odds', 0)
            a = e.get('away_odds', 0)
            if h > best_home['odds'] and h > 1.01:
                best_home = {'odds': h, 'event': e}
            if a > best_away['odds'] and a > 1.01:
                best_away = {'odds': a, 'event': e}

        if not best_home['event'] or not best_away['event']:
            return None
        if best_home['event'] is best_away['event']:
            return None

        margin = 1 / best_home['odds'] + 1 / best_away['odds']
        if margin >= 1:
            return None

        profit = (1 / margin - 1) * 100
        if profit < self.min_profit * 100:
            return None

        return self._build_surebet(
            match_key=match_key,
            market_type='2-way',
            profit=profit,
            legs=[
                {
                    'bookmaker': best_home['event']['bookmaker'],
                    'selection': 'П1',
                    'odds': best_home['odds'],
                },
                {
                    'bookmaker': best_away['event']['bookmaker'],
                    'selection': 'П2',
                    'odds': best_away['odds'],
                },
            ],
        )

    def _detect_3way_fast(self, events: List[Dict], match_key: str) -> Optional[Dict]:
        """Ultra-fast 3-way surebet detection."""
        best = {
            'home': {'odds': 0, 'event': None},
            'draw': {'odds': 0, 'event': None},
            'away': {'odds': 0, 'event': None},
        }

        for e in events:
            h = e.get('home_odds', 0) or 0
            d = e.get('draw_odds', 0) or 0
            a = e.get('away_odds', 0) or 0

            if h > best['home']['odds'] and h > 1.01:
                best['home'] = {'odds': h, 'event': e}
            if d > best['draw']['odds'] and d > 1.01:
                best['draw'] = {'odds': d, 'event': e}
            if a > best['away']['odds'] and a > 1.01:
                best['away'] = {'odds': a, 'event': e}

        if not all(best[k]['event'] for k in ['home', 'draw', 'away']):
            return None

        bookmakers = {best[k]['event']['bookmaker'] for k in ['home', 'draw', 'away']}
        if len(bookmakers) < 2:
            return None

        margin = sum(1 / best[k]['odds'] for k in ['home', 'draw', 'away'])
        if margin >= 1:
            return None

        profit = (1 / margin - 1) * 100
        if profit < self.min_profit * 100:
            return None

        return self._build_surebet(
            match_key=match_key,
            market_type='3-way',
            profit=profit,
            legs=[
                {
                    'bookmaker': best['home']['event']['bookmaker'],
                    'selection': 'П1',
                    'odds': best['home']['odds'],
                },
                {
                    'bookmaker': best['draw']['event']['bookmaker'],
                    'selection': 'X',
                    'odds': best['draw']['odds'],
                },
                {
                    'bookmaker': best['away']['event']['bookmaker'],
                    'selection': 'П2',
                    'odds': best['away']['odds'],
                },
            ],
        )

    def _build_surebet(
        self,
        match_key: str,
        market_type: str,
        profit: float,
        legs: List[Dict],
    ) -> Dict:
        """Build surebet result dict."""
        bookmakers = [leg['bookmaker'] for leg in legs]
        surebet_id = hashlib.md5(
            f"{match_key}|{'|'.join(sorted(bookmakers))}|{market_type}|{time.time()}".encode()
        ).hexdigest()[:8]

        total_stake = 10000.0
        inverses = [1.0 / leg['odds'] for leg in legs]
        total_inverse = sum(inverses)
        stakes = [(total_stake * inv / total_inverse) for inv in inverses]

        return {
            'id': surebet_id,
            'event_name': match_key.replace('|', ' vs '),
            'sport': 'football',
            'market_type': market_type,
            'is_live': True,
            'is_momentum': True,
            'profit_percent': profit,
            'total_stake': total_stake,
            'estimated_profit': total_stake * (1 / (1 / (1 + profit / 100)) - 1),
            'legs': [
                {**leg, 'calculated_stake': stakes[i], 'stake_percent': stakes[i] / total_stake * 100}
                for i, leg in enumerate(legs)
            ],
            'bookmakers': list(set(bookmakers)),
            'found_at': datetime.utcnow().isoformat(),
        }

    def get_stats(self) -> Dict:
        return {
            'detection_count': self._detection_count,
            'surebets_found': self._success_count,
            'success_rate': round(
                self._success_count / max(1, self._detection_count) * 100, 2
            ),
        }


# ───────────────────────────────────────────────────────────────
# Priority Notification System
# ───────────────────────────────────────────────────────────────

class PriorityNotifier:
    """
    High-priority notification system for momentum surebets.
    Bypasses normal filtering and sends immediate alerts.
    """

    def __init__(self):
        self._subscribers: List[Callable] = []
        self._telegram_callback: Optional[Callable] = None
        self._ws_callback: Optional[Callable] = None
        self._sound_alert: bool = True  # Enable sound alerts for momentum surebets

    def subscribe(self, callback: Callable):
        self._subscribers.append(callback)

    def set_telegram_callback(self, callback: Callable):
        self._telegram_callback = callback

    def set_ws_callback(self, callback: Callable):
        self._ws_callback = callback

    async def notify(self, surebets: List[Dict], trigger: MomentumTrigger):
        """Send priority notifications for momentum surebets."""
        if not surebets:
            return

        for sb in surebets:
            sb['priority'] = 'HIGH'
            sb['trigger_type'] = trigger.trigger_type.value
            sb['alert_type'] = 'momentum_surebet'

        # Notify all subscribers immediately
        for callback in self._subscribers:
            try:
                if asyncio.iscoroutinefunction(callback):
                    await callback(surebets)
                else:
                    callback(surebets)
            except Exception as e:
                logger.error(f"Momentum notification error: {e}")

        # Telegram with special formatting
        if self._telegram_callback:
            for sb in surebets:
                try:
                    await self._telegram_callback(sb, priority=True)
                except Exception as e:
                    logger.error(f"Telegram momentum notification error: {e}")

        # WebSocket real-time broadcast
        if self._ws_callback:
            try:
                await self._ws_callback(surebets)
            except Exception as e:
                logger.error(f"WS momentum notification error: {e}")

        logger.info(
            f"🚨 PRIORITY ALERT: {len(surebets)} momentum surebet(s) | "
            f"Trigger: {trigger.trigger_type.value} | "
            f"Match: {trigger.match_key}"
        )


# ───────────────────────────────────────────────────────────────
# Main Momentum Scanner
# ───────────────────────────────────────────────────────────────

class MomentumScanner:
    """
    Live Momentum Scanner - Main orchestrator.

    Integrates with GhostScanner engine to provide trigger-based
    high-profit surebet detection during match events.

    Usage:
        scanner = MomentumScanner()
        await scanner.initialize()
        ghost_scanner.momentum_scanner = scanner  # Hook into main engine
        await scanner.start()
    """

    def __init__(
        self,
        min_profit: float = 5.0,
        max_concurrent_windows: int = 5,
        scan_interval: float = 0.5,
    ):
        self.min_profit = min_profit
        self.max_concurrent_windows = max_concurrent_windows
        self.scan_interval = scan_interval

        # Core components
        self.feed_connector = LiveFeedConnector()
        self.odds_tracker = RapidOddsTracker()
        self.surebet_detector = MomentumSurebetDetector(min_profit=min_profit)
        self.notifier = PriorityNotifier()

        # State
        self.is_running: bool = False
        self._active_windows: Dict[str, MomentumWindow] = {}
        self._pending_triggers: asyncio.Queue = asyncio.Queue()
        self._total_triggers: int = 0
        self._total_momentum_surebets: int = 0
        self._started_at: Optional[float] = None

        # Integration hooks (set by GhostScanner)
        self._event_fetch_callback: Optional[Callable] = None
        self._database = None

    async def initialize(self):
        """Initialize momentum scanner components."""
        await self.feed_connector.connect()
        self.feed_connector.on_trigger(self._on_trigger_received)
        logger.info("⚡ MomentumScanner initialized")

    async def start(self):
        """Start the momentum scanner."""
        if self.is_running:
            return

        self.is_running = True
        self._started_at = time.time()

        asyncio.create_task(self._trigger_processor())
        asyncio.create_task(self._window_manager())
        asyncio.create_task(self._odds_spike_detector())

        logger.info("⚡ MomentumScanner started")

    async def stop(self):
        """Stop the momentum scanner."""
        self.is_running = False

        # Close all active windows
        for window in self._active_windows.values():
            window.close()
        self._active_windows.clear()

        await self.feed_connector.disconnect()
        logger.info("⏹️ MomentumScanner stopped")

    # ─── Trigger Handling ───

    async def _on_trigger_received(self, trigger: MomentumTrigger):
        """Handle incoming trigger from live feed."""
        await self._pending_triggers.put(trigger)
        self._total_triggers += 1
        logger.debug(
            f"🎯 Trigger received: {trigger.trigger_type.value} "
            f"on {trigger.match_key}"
        )

    async def inject_trigger(self, trigger: MomentumTrigger):
        """Manually inject a trigger (for testing or external integration)."""
        await self._pending_triggers.put(trigger)

    async def _trigger_processor(self):
        """Process incoming triggers and create momentum windows."""
        while self.is_running:
            try:
                trigger = await asyncio.wait_for(
                    self._pending_triggers.get(), timeout=1.0
                )

                # Check if we can accept more windows
                if len(self._active_windows) >= self.max_concurrent_windows:
                    # Close lowest priority window if needed
                    self._evict_lowest_priority_window()

                # Create new momentum window
                window = MomentumWindow(
                    trigger=trigger,
                    scan_interval=self.scan_interval,
                    min_profit=self.min_profit,
                )
                self._active_windows[trigger.id] = window

                logger.info(
                    f"🔥 MOMENTUM WINDOW OPENED: {trigger.trigger_type.value} "
                    f"on {trigger.match_key} | "
                    f"Duration: {trigger.window_duration}s | "
                    f"Active windows: {len(self._active_windows)}"
                )

            except asyncio.TimeoutError:
                continue
            except Exception as e:
                logger.error(f"Trigger processor error: {e}")

    async def _window_manager(self):
        """Manage active momentum windows and run rapid scans."""
        while self.is_running:
            try:
                expired = []

                for window_id, window in list(self._active_windows.items()):
                    if not window.should_scan():
                        expired.append(window_id)
                        window.close()
                        continue

                    # Run rapid scan for this window
                    await self._scan_window(window)

                # Clean up expired windows
                for window_id in expired:
                    del self._active_windows[window_id]

                await asyncio.sleep(0.1)  # Small sleep between iterations

            except Exception as e:
                logger.error(f"Window manager error: {e}")
                await asyncio.sleep(1)

    async def _scan_window(self, window: MomentumWindow):
        """Run a single rapid scan within a momentum window."""
        if not self._event_fetch_callback:
            return

        try:
            # Fetch latest events from main scanner
            events = await self._event_fetch_callback()

            if not events:
                return

            # Track odds changes
            for event in events:
                key = f"{event.get('home_team', '')}|{event.get('away_team', '')}|{event.get('bookmaker', '')}"
                self.odds_tracker.update(key, event)

            # Record snapshot
            window.record_snapshot({'event_count': len(events)})
            window.scan_count += 1

            # Detect surebets
            surebets = self.surebet_detector.detect(
                events=events,
                trigger=window.trigger,
            )

            if surebets:
                window.surebets_found += len(surebets)
                self._total_momentum_surebets += len(surebets)

                # Send priority notifications
                await self.notifier.notify(surebets, window.trigger)

        except Exception as e:
            logger.error(f"Window scan error: {e}")

    async def _odds_spike_detector(self):
        """Detect momentum triggers from odds movement patterns."""
        while self.is_running:
            try:
                # Check all tracked events for spikes
                for event_key in list(self.odds_tracker._odds_history.keys()):
                    if self.odds_tracker.detect_spike(event_key, threshold=0.15):
                        # Extract match key from event key
                        parts = event_key.split('|')
                        if len(parts) >= 2:
                            match_key = f"{parts[0]}|{parts[1]}"
                            trigger = MomentumTrigger(
                                trigger_type=MomentumTriggerType.ODDS_SPIKE,
                                match_key=match_key,
                                source='odds_tracker',
                                confidence=0.7,
                            )
                            await self._pending_triggers.put(trigger)

                await asyncio.sleep(1.0)

            except Exception as e:
                logger.error(f"Odds spike detector error: {e}")
                await asyncio.sleep(2)

    def _evict_lowest_priority_window(self):
        """Close the lowest priority active window to make room."""
        if not self._active_windows:
            return

        lowest_id = min(
            self._active_windows.keys(),
            key=lambda wid: self._active_windows[wid].trigger.priority,
        )
        window = self._active_windows.pop(lowest_id)
        window.close()
        logger.debug(f"Evicted low-priority window: {lowest_id}")

    # ─── Integration with GhostScanner ───

    def set_event_fetch_callback(self, callback: Callable):
        """Set callback to fetch events from main scanner."""
        self._event_fetch_callback = callback

    def set_database(self, database):
        """Set database reference for saving momentum surebets."""
        self._database = database

    def hook_into_engine(self, engine):
        """
        Hook momentum scanner into GhostScanner engine.
        This is the main integration point.
        """
        # Set event fetch callback
        self.set_event_fetch_callback(engine._fetch_all_events)
        self.set_database(engine.database)

        # Hook notifications into engine's subscriber system
        self.notifier.subscribe(engine._notify_subscribers)

        # Hook Telegram notifications
        if engine._telegram_callback:
            self.notifier.set_telegram_callback(engine._telegram_callback)

        # Hook WebSocket notifications
        try:
            from api.websocket import ws_manager

            async def ws_broadcast(surebets):
                for sb in surebets:
                    await ws_manager.send_new_surebet(sb)

            self.notifier.set_ws_callback(ws_broadcast)
        except Exception:
            pass

        logger.info("⚡ MomentumScanner hooked into GhostScanner engine")

    # ─── Statistics ───

    def get_stats(self) -> Dict:
        uptime = time.time() - self._started_at if self._started_at else 0
        return {
            'is_running': self.is_running,
            'uptime_seconds': uptime,
            'active_windows': len(self._active_windows),
            'max_concurrent_windows': self.max_concurrent_windows,
            'total_triggers': self._total_triggers,
            'total_momentum_surebets': self._total_momentum_surebets,
            'min_profit': self.min_profit,
            'scanner_stats': self.surebet_detector.get_stats(),
            'active_window_details': [
                {
                    'trigger': w.trigger.trigger_type.value,
                    'match': w.trigger.match_key,
                    'time_remaining': round(w.time_remaining, 1),
                    'scans': w.scan_count,
                    'surebets': w.surebets_found,
                }
                for w in self._active_windows.values()
            ],
        }


# ───────────────────────────────────────────────────────────────
# Integration Example (for GhostScanner engine.py)
# ───────────────────────────────────────────────────────────────
"""
To integrate MomentumScanner into GhostScanner, add to engine.py:

1. Import:
   from core.momentum_scanner import MomentumScanner

2. In __init__:
   self.momentum_scanner = MomentumScanner(min_profit=5.0)

3. In start():
   await self.momentum_scanner.initialize()
   self.momentum_scanner.hook_into_engine(self)
   await self.momentum_scanner.start()

4. In stop():
   await self.momentum_scanner.stop()

5. In get_stats():
   stats['momentum_scanner'] = self.momentum_scanner.get_stats()
"""
