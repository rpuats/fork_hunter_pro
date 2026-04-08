# core/event_pool.py
"""
Lock-free event pool with O(1) lookup, incremental updates, and memory-bounded storage.
Uses hash-based indexing and atomic operations for maximum throughput.
"""
import time
import hashlib
import threading
from typing import Dict, List, Optional, Tuple, Any
from collections import OrderedDict
from dataclasses import dataclass, field
import logging

logger = logging.getLogger(__name__)


@dataclass
class EventEntry:
    """Lightweight event entry with hash-based lookup."""
    key: str
    hash_value: int
    data: Dict
    timestamp: float
    version: int = 0

    def update(self, new_data: Dict) -> bool:
        """Update if data changed. Returns True if changed."""
        new_odds_hash = hash(
            str(new_data.get('home_odds', 0)) +
            str(new_data.get('away_odds', 0)) +
            str(new_data.get('draw_odds', 0))
        )
        old_odds_hash = self.hash_value
        if new_odds_hash != old_odds_hash:
            self.data = new_data
            self.hash_value = new_odds_hash
            self.timestamp = time.time()
            self.version += 1
            return True
        return False


class BloomFilter:
    """Space-efficient probabilistic membership test for duplicate detection."""

    def __init__(self, capacity: int = 100000, error_rate: float = 0.01):
        import math
        self.size = int(-capacity * math.log(error_rate) / (math.log(2) ** 2))
        self.hash_count = int(self.size / capacity * math.log(2))
        self.bit_array = [False] * self.size
        self._size = self.size
        self._hash_count = self.hash_count

    def _get_hashes(self, item: str) -> List[int]:
        hashes = []
        for i in range(self._hash_count):
            h = hashlib.md5(f"{item}{i}".encode()).hexdigest()
            hashes.append(int(h, 16) % self._size)
        return hashes

    def add(self, item: str):
        for h in self._get_hashes(item):
            self.bit_array[h] = True

    def might_contain(self, item: str) -> bool:
        return all(self.bit_array[h] for h in self._get_hashes(item))

    def clear(self):
        self.bit_array = [False] * self._size


class EventPool:
    """
    High-performance event storage with:
    - O(1) lookup by normalized key hash
    - Incremental change detection
    - Memory-bounded with LRU eviction
    - Thread-safe with minimal locking
    """

    def __init__(
        self,
        max_size: int = 50000,
        eviction_threshold: float = 0.9,
        stale_ttl: float = 300.0,
    ):
        self.max_size = max_size
        self.eviction_threshold = eviction_threshold
        self.stale_ttl = stale_ttl

        self._pool: OrderedDict[str, EventEntry] = OrderedDict()
        self._lock = threading.RLock()

        self._bloom = BloomFilter(capacity=max_size)
        self._bloom_reset_count = 0

        self._stats = {
            'inserts': 0,
            'updates': 0,
            'evictions': 0,
            'stale_cleanups': 0,
            'bloom_hits': 0,
            'bloom_misses': 0,
        }

    def _make_key(self, event: Dict) -> str:
        """Create normalized event key."""
        home = event.get('home_team', '').lower().strip()
        away = event.get('away_team', '').lower().strip()
        bookmaker = event.get('bookmaker', '')
        market = event.get('market', '1x2')
        if home and away and bookmaker:
            return f"{home}|{away}|{bookmaker}|{market}"
        return ""

    def _odds_hash(self, event: Dict) -> int:
        """Fast hash of odds values only for change detection."""
        return hash(
            str(event.get('home_odds', 0)) +
            str(event.get('away_odds', 0)) +
            str(event.get('draw_odds', 0))
        )

    def upsert(self, event: Dict) -> Tuple[bool, bool]:
        """
        Insert or update event.
        Returns (was_inserted, was_changed).
        """
        key = self._make_key(event)
        if not key:
            return False, False

        with self._lock:
            if key in self._pool:
                entry = self._pool[key]
                self._pool.move_to_end(key)
                changed = entry.update(event)
                if changed:
                    self._stats['updates'] += 1
                return False, changed
            else:
                if len(self._pool) >= self.max_size:
                    self._evict()

                entry = EventEntry(
                    key=key,
                    hash_value=self._odds_hash(event),
                    data=event,
                    timestamp=time.time(),
                )
                self._pool[key] = entry
                self._bloom.add(key)
                self._stats['inserts'] += 1
                return True, True

    def upsert_batch(self, events: List[Dict]) -> Tuple[int, int]:
        """
        Batch upsert for maximum throughput.
        Returns (inserted_count, changed_count).
        """
        inserted = 0
        changed = 0
        with self._lock:
            for event in events:
                key = self._make_key(event)
                if not key:
                    continue

                if key in self._pool:
                    entry = self._pool[key]
                    self._pool.move_to_end(key)
                    if entry.update(event):
                        changed += 1
                        self._stats['updates'] += 1
                else:
                    if len(self._pool) >= self.max_size:
                        self._evict()
                    entry = EventEntry(
                        key=key,
                        hash_value=self._odds_hash(event),
                        data=event,
                        timestamp=time.time(),
                    )
                    self._pool[key] = entry
                    self._bloom.add(key)
                    inserted += 1
                    self._stats['inserts'] += 1
        return inserted, changed

    def get_changed(self, events: List[Dict]) -> List[Dict]:
        """
        Return only events that have changed since last seen.
        Returns full list if pool is empty (first run).
        """
        if not self._pool:
            return events

        changed = []
        with self._lock:
            current_keys = set()
            for event in events:
                key = self._make_key(event)
                if not key:
                    continue
                current_keys.add(key)

                if key not in self._pool:
                    changed.append(event)
                else:
                    entry = self._pool[key]
                    new_hash = self._odds_hash(event)
                    if entry.hash_value != new_hash:
                        changed.append(event)
                        entry.update(event)

            removed = set(self._pool.keys()) - current_keys
            for key in removed:
                del self._pool[key]

        return changed

    def get(self, key: str) -> Optional[Dict]:
        """Get event by key."""
        with self._lock:
            entry = self._pool.get(key)
            if entry:
                self._pool.move_to_end(key)
                return entry.data
            return None

    def get_all(self) -> List[Dict]:
        """Get all events as list."""
        with self._lock:
            return [entry.data for entry in self._pool.values()]

    def get_count(self) -> int:
        """Get current pool size."""
        return len(self._pool)

    def _evict(self):
        """Evict oldest entries when pool is full."""
        evict_count = int(self.max_size * (1 - self.eviction_threshold))
        for _ in range(max(evict_count, 100)):
            if self._pool:
                self._pool.popitem(last=False)
                self._stats['evictions'] += 1
            else:
                break

    def cleanup_stale(self) -> int:
        """Remove events older than stale_ttl."""
        now = time.time()
        stale_keys = []
        with self._lock:
            for key, entry in self._pool.items():
                if now - entry.timestamp > self.stale_ttl:
                    stale_keys.append(key)
            for key in stale_keys:
                del self._pool[key]
            self._stats['stale_cleanups'] += len(stale_keys)
        return len(stale_keys)

    def might_have(self, event: Dict) -> bool:
        """Quick bloom filter check."""
        key = self._make_key(event)
        if not key:
            return False
        result = self._bloom.might_contain(key)
        if result:
            self._stats['bloom_hits'] += 1
        else:
            self._stats['bloom_misses'] += 1
        return result

    def reset_bloom(self):
        """Reset bloom filter when too many false positives expected."""
        self._bloom.clear()
        self._bloom_reset_count += 1
        with self._lock:
            for key in self._pool:
                self._bloom.add(key)

    def stats(self) -> Dict:
        """Get pool statistics."""
        return {
            'size': len(self._pool),
            'max_size': self.max_size,
            'utilization': round(len(self._pool) / self.max_size * 100, 1),
            'bloom_resets': self._bloom_reset_count,
            **self._stats,
        }

    def clear(self):
        """Clear all events."""
        with self._lock:
            self._pool.clear()
            self._bloom.clear()
