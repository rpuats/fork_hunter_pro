# core/finder_optimized.py
"""
Optimized surebet detection with:
- NumPy vectorized margin calculations (fallback to pure Python)
- Bloom filter for fast duplicate detection
- Parallel detection using ProcessPoolExecutor for CPU-bound work
"""
import uuid
import time
import hashlib
from typing import List, Dict, Optional, Tuple
from datetime import datetime
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor
import logging
import threading

logger = logging.getLogger(__name__)

try:
    import numpy as np  # type: ignore[import-not-found]
    HAS_NUMPY = True
except ImportError:
    HAS_NUMPY = False
    np = None  # type: ignore[assignment]

from core.finder import SurebetLeg


class SurebetBloomFilter:
    """Bloom filter for fast surebet duplicate detection."""

    def __init__(self, capacity: int = 50000, error_rate: float = 0.001):
        import math
        self.size = int(-capacity * math.log(error_rate) / (math.log(2) ** 2))
        self.hash_count = int(self.size / capacity * math.log(2))
        self.bit_array = bytearray(self.size // 8 + 1)
        self._added = 0

    def _get_hashes(self, item: str) -> List[int]:
        hashes = []
        for i in range(self.hash_count):
            h = hashlib.sha256(f"{item}{i}".encode()).hexdigest()
            hashes.append(int(h[:8], 16) % self.size)
        return hashes

    def add(self, item: str):
        for h in self._get_hashes(item):
            byte_idx = h // 8
            bit_idx = h % 8
            self.bit_array[byte_idx] |= (1 << bit_idx)
        self._added += 1

    def might_contain(self, item: str) -> bool:
        for h in self._get_hashes(item):
            byte_idx = h // 8
            bit_idx = h % 8
            if not (self.bit_array[byte_idx] & (1 << bit_idx)):
                return False
        return True

    @property
    def count(self) -> int:
        return self._added


class VectorizedCalculator:
    """NumPy-accelerated margin calculations."""

    def __init__(self):
        self._cache: Dict[str, float] = {}
        self._hits = 0
        self._misses = 0

    def _make_key(self, odds: List[float]) -> str:
        return ','.join(f'{o:.4f}' for o in odds)

    def calculate_margin(self, odds: List[float]) -> float:
        key = self._make_key(odds)
        if key in self._cache:
            self._hits += 1
            return self._cache[key]
        self._misses += 1

        if HAS_NUMPY and len(odds) >= 4:
            arr = np.array(odds, dtype=np.float64)  # type: ignore[union-attr]
            result = float(np.sum(1.0 / arr))  # type: ignore[union-attr]
        else:
            result = sum(1.0 / o for o in odds)

        self._cache[key] = result
        return result

    def calculate_margins_batch(self, odds_list: List[List[float]]) -> List[float]:
        """Vectorized batch margin calculation."""
        if HAS_NUMPY and len(odds_list) >= 10 and np is not None:
            max_len = max(len(o) for o in odds_list)
            padded = [o + [1.0] * (max_len - len(o)) for o in odds_list]
            arr = np.array(padded, dtype=np.float64)
            margins = np.sum(1.0 / arr, axis=1)
            result = margins.tolist()
        else:
            result = [self.calculate_margin(o) for o in odds_list]
        return result

    def stats(self) -> Dict:
        total = self._hits + self._misses
        return {
            'hits': self._hits,
            'misses': self._misses,
            'hit_rate': round(self._hits / total * 100, 2) if total > 0 else 0,
            'numpy_enabled': HAS_NUMPY,
        }


class OptimizedSurebetCalculator:
    """
    High-performance surebet detector with:
    - Vectorized calculations
    - Bloom filter dedup
    - Single-pass event grouping
    """

    def __init__(self, min_profit: float = 0.5):
        self.min_profit = min_profit / 100
        self._calc = VectorizedCalculator()
        self._bloom = SurebetBloomFilter()
        self._lock = threading.Lock()

    def _make_surebet_id(self, event_name: str, bookmakers: List[str], market: str) -> str:
        raw = f"{event_name}|{'|'.join(sorted(bookmakers))}|{market}"
        return hashlib.md5(raw.encode()).hexdigest()[:8]

    def _group_events(self, events: List[Dict]) -> Dict[tuple, List[Dict]]:
        grouped: Dict[tuple, List[Dict]] = defaultdict(list)
        for event in events:
            home = event.get('home_team', '').lower().strip()
            away = event.get('away_team', '').lower().strip()
            if not home or not away:
                continue
            key = tuple(sorted([home, away]))
            grouped[key].append(event)
        return grouped

    def _build_surebet(
        self,
        event_name: str,
        sport: str,
        market_type: str,
        is_live: bool,
        odds: List[float],
        legs_data: List[Dict],
    ) -> Optional[Dict]:
        margin = self._calc.calculate_margin(odds)
        if margin >= 1:
            return None

        profit = (1 / margin - 1) * 100
        if profit < self.min_profit * 100:
            return None

        total_stake = 10000.0
        inverses = [1.0 / o for o in odds]
        total_inverse = sum(inverses)
        stakes = [(total_stake * inv / total_inverse) for inv in inverses]

        bookmakers = [l['bookmaker'] for l in legs_data]
        surebet_id = self._make_surebet_id(event_name, bookmakers, market_type)

        with self._lock:
            if self._bloom.might_contain(surebet_id):
                return None
            self._bloom.add(surebet_id)

        legs = []
        for i, leg_data in enumerate(legs_data):
            legs.append({
                'bookmaker': leg_data['bookmaker'],
                'market': leg_data['market'],
                'selection': leg_data['selection'],
                'odds': odds[i],
                'event_name': event_name,
                'calculated_stake': stakes[i],
                'stake_percent': stakes[i] / total_stake * 100,
            })

        return {
            'id': surebet_id,
            'event_name': event_name,
            'sport': sport,
            'market_type': market_type,
            'is_live': is_live,
            'profit_percent': profit,
            'total_stake': total_stake,
            'estimated_profit': total_stake * (1 / margin - 1),
            'legs': legs,
            'bookmakers': list(set(bookmakers)),
            'found_at': datetime.utcnow().isoformat(),
        }

    def find_2way_surebets(self, events: List[Dict]) -> List[Dict]:
        surebets = []
        events_by_key = self._group_events(events)

        for key, same_events in events_by_key.items():
            if len(same_events) < 2:
                continue

            home_best = None
            home_best_odds = 0.0
            away_best = None
            away_best_odds = 0.0

            for e in same_events:
                h_odds = e.get('home_odds', 0)
                a_odds = e.get('away_odds', 0)

                if h_odds > home_best_odds and h_odds > 1.01:
                    home_best_odds = h_odds
                    home_best = e

                if a_odds > away_best_odds and a_odds > 1.01:
                    away_best_odds = a_odds
                    away_best = e

            if not home_best or not away_best or home_best is away_best:
                continue

            event_name = f"{home_best.get('home_team')} vs {home_best.get('away_team')}"
            result = self._build_surebet(
                event_name=event_name,
                sport=home_best.get('sport', 'football'),
                market_type='2-way',
                is_live=home_best.get('is_live', True),
                odds=[home_best_odds, away_best_odds],
                legs_data=[
                    {
                        'bookmaker': home_best['bookmaker'],
                        'market': '1',
                        'selection': 'П1',
                    },
                    {
                        'bookmaker': away_best['bookmaker'],
                        'market': '2',
                        'selection': 'П2',
                    },
                ],
            )
            if result:
                surebets.append(result)

        return surebets

    def find_3way_surebets(self, events: List[Dict]) -> List[Dict]:
        surebets = []
        events_by_key = self._group_events(events)

        for key, same_events in events_by_key.items():
            best_home_odds = 0.0
            best_home = None
            best_draw_odds = 0.0
            best_draw = None
            best_away_odds = 0.0
            best_away = None

            for e in same_events:
                h = e.get('home_odds') or 0
                d = e.get('draw_odds') or 0
                a = e.get('away_odds') or 0

                if h > best_home_odds and h > 1.01:
                    best_home_odds = h
                    best_home = e
                if d > best_draw_odds and d > 1.01:
                    best_draw_odds = d
                    best_draw = e
                if a > best_away_odds and a > 1.01:
                    best_away_odds = a
                    best_away = e

            if not best_home or not best_draw or not best_away:
                continue

            event_name = f"{best_home.get('home_team')} vs {best_home.get('away_team')}"
            result = self._build_surebet(
                event_name=event_name,
                sport=best_home.get('sport', 'football'),
                market_type='3-way',
                is_live=best_home.get('is_live', True),
                odds=[best_home_odds, best_draw_odds, best_away_odds],
                legs_data=[
                    {
                        'bookmaker': best_home['bookmaker'],
                        'market': '1',
                        'selection': 'П1',
                    },
                    {
                        'bookmaker': best_draw['bookmaker'],
                        'market': 'X',
                        'selection': 'Ничья',
                    },
                    {
                        'bookmaker': best_away['bookmaker'],
                        'market': '2',
                        'selection': 'П2',
                    },
                ],
            )
            if result:
                surebets.append(result)

        return surebets

    def find_surebets(self, events: List[Dict]) -> List[Dict]:
        all_surebets = []
        all_surebets.extend(self.find_2way_surebets(events))
        all_surebets.extend(self.find_3way_surebets(events))
        return sorted(all_surebets, key=lambda x: x.get('profit_percent', 0), reverse=True)

    def get_stats(self) -> Dict:
        return {
            'calc_cache': self._calc.stats(),
            'bloom_count': self._bloom.count,
        }


def _detect_surebets_worker(args: Tuple[List[Dict], float, str]) -> List[Dict]:
    """Worker function for ProcessPoolExecutor."""
    events, min_profit, mode = args
    calc = OptimizedSurebetCalculator(min_profit=min_profit)
    if mode == '2way':
        return calc.find_2way_surebets(events)
    elif mode == '3way':
        return calc.find_3way_surebets(events)
    return []


class ParallelSurebetDetector:
    """
    Parallel surebet detection using ProcessPoolExecutor.
    Splits work across processes for CPU-bound detection.
    Falls back to threaded execution on Windows.
    """

    def __init__(self, min_profit: float = 0.5, max_workers: int = 2):
        self.min_profit = min_profit
        self.max_workers = max_workers
        self._executor: Optional[ProcessPoolExecutor] = None

    def _get_executor(self) -> Optional[ProcessPoolExecutor]:
        if self._executor is None:
            try:
                self._executor = ProcessPoolExecutor(max_workers=self.max_workers)
            except (RuntimeError, OSError):
                pass
        return self._executor

    def find_surebets_parallel(self, events: List[Dict]) -> List[Dict]:
        """Run 2-way and 3-way detection in parallel processes."""
        executor = self._get_executor()
        if executor is None:
            calc = OptimizedSurebetCalculator(min_profit=self.min_profit)
            return calc.find_surebets(events)

        try:
            future_2way = executor.submit(
                _detect_surebets_worker,
                (events, self.min_profit, '2way')
            )
            future_3way = executor.submit(
                _detect_surebets_worker,
                (events, self.min_profit, '3way')
            )

            results_2way = future_2way.result(timeout=10)
            results_3way = future_3way.result(timeout=10)

            all_surebets = results_2way + results_3way
            return sorted(all_surebets, key=lambda x: x.get('profit_percent', 0), reverse=True)
        except Exception:
            calc = OptimizedSurebetCalculator(min_profit=self.min_profit)
            return calc.find_surebets(events)

    def shutdown(self):
        if self._executor:
            self._executor.shutdown(wait=False)
            self._executor = None
