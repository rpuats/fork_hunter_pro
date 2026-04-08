# core/memory_manager.py
"""
Memory management with:
- tracemalloc integration for leak detection
- Memory limits with auto-cleanup
- Object pooling for frequently created objects
"""
import gc
import sys
import time
import tracemalloc
import logging
import threading
from typing import Dict, List, Optional, Any, Callable
from collections import defaultdict
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


@dataclass
class MemorySnapshot:
    timestamp: float
    current_mb: float
    peak_mb: float
    gc_objects: int
    top_allocations: List[tuple] = field(default_factory=list)


class ObjectPool:
    """
    Object pool for frequently created/destroyed objects.
    Reduces GC pressure and allocation overhead.
    """

    def __init__(self, factory: Callable, max_size: int = 1000, name: str = "unnamed"):
        self._factory = factory
        self._max_size = max_size
        self._name = name
        self._pool: List[Any] = []
        self._lock = threading.Lock()
        self._created = 0
        self._reused = 0

    def acquire(self) -> Any:
        with self._lock:
            if self._pool:
                self._reused += 1
                return self._pool.pop()
            self._created += 1
            return self._factory()

    def release(self, obj: Any):
        with self._lock:
            if len(self._pool) < self._max_size:
                self._pool.append(obj)

    def stats(self) -> Dict:
        return {
            'name': self._name,
            'pool_size': len(self._pool),
            'max_size': self._max_size,
            'created': self._created,
            'reused': self._reused,
            'reuse_rate': round(self._reused / max(self._created + self._reused, 1) * 100, 1),
        }

    def clear(self):
        with self._lock:
            self._pool.clear()


class MemoryManager:
    """
    Memory manager with leak detection and auto-cleanup.
    """

    def __init__(
        self,
        memory_limit_mb: float = 512.0,
        cleanup_threshold_mb: float = 400.0,
        snapshot_interval: int = 100,
    ):
        self.memory_limit_mb = memory_limit_mb
        self.cleanup_threshold_mb = cleanup_threshold_mb
        self.snapshot_interval = snapshot_interval

        self._started = False
        self._snapshots: List[MemorySnapshot] = []
        self._max_snapshots = 100
        self._cycle_count = 0
        self._cleanup_count = 0
        self._lock = threading.Lock()

        self._object_pools: Dict[str, ObjectPool] = {}

        self._callbacks: List[Callable] = []

    def start(self):
        if not self._started:
            tracemalloc.start(25)
            self._started = True
            logger.info(f"Memory manager started. Limit: {self.memory_limit_mb}MB")

    def stop(self):
        if self._started:
            tracemalloc.stop()
            self._started = False
            logger.info("Memory manager stopped")

    def on_cycle(self):
        """Call at the end of each scanner cycle."""
        self._cycle_count += 1
        if self._cycle_count % self.snapshot_interval == 0:
            self._take_snapshot()
            self._check_memory_limits()

    def _take_snapshot(self):
        if not self._started:
            return

        current, peak = tracemalloc.get_traced_memory()
        snapshot = tracemalloc.take_snapshot()
        top_stats = snapshot.statistics('lineno')[:10]

        with self._lock:
            snap = MemorySnapshot(
                timestamp=time.time(),
                current_mb=round(current / 1024 / 1024, 2),
                peak_mb=round(peak / 1024 / 1024, 2),
                gc_objects=len(gc.get_objects()),
                top_allocations=[(str(s.traceback), s.size) for s in top_stats],
            )
            self._snapshots.append(snap)
            if len(self._snapshots) > self._max_snapshots:
                self._snapshots.pop(0)

    def _check_memory_limits(self):
        if not self._started:
            return

        current, peak = tracemalloc.get_traced_memory()
        current_mb = current / 1024 / 1024

        if current_mb > self.memory_limit_mb:
            logger.warning(f"Memory limit exceeded: {current_mb:.1f}MB > {self.memory_limit_mb}MB")
            self._force_cleanup()
            for cb in self._callbacks:
                try:
                    cb('memory_limit_exceeded', current_mb)
                except Exception as e:
                    logger.error(f"Memory callback error: {e}")

        elif current_mb > self.cleanup_threshold_mb:
            logger.info(f"Memory above threshold: {current_mb:.1f}MB, running cleanup")
            self._force_cleanup()

    def _force_cleanup(self):
        gc.collect()
        for pool in self._object_pools.values():
            pool.clear()
        self._cleanup_count += 1
        logger.info(f"Forced cleanup #{self._cleanup_count}")

    def register_pool(self, name: str, pool: ObjectPool):
        self._object_pools[name] = pool

    def register_callback(self, callback: Callable):
        self._callbacks.append(callback)

    def get_memory_usage(self) -> Dict:
        if self._started:
            current, peak = tracemalloc.get_traced_memory()
            return {
                'current_mb': round(current / 1024 / 1024, 2),
                'peak_mb': round(peak / 1024 / 1024, 2),
                'limit_mb': self.memory_limit_mb,
                'utilization_percent': round(current / (self.memory_limit_mb * 1024 * 1024) * 100, 1),
                'gc_objects': len(gc.get_objects()),
                'gc_generations': gc.get_count(),
            }
        return {'current_mb': 0, 'peak_mb': 0}

    def get_top_allocations(self, count: int = 10) -> List[str]:
        if not self._started:
            return []
        snapshot = tracemalloc.take_snapshot()
        top_stats = snapshot.statistics('lineno')[:count]
        return [str(stat) for stat in top_stats]

    def get_pool_stats(self) -> Dict[str, Dict]:
        return {name: pool.stats() for name, pool in self._object_pools.items()}

    def get_report(self) -> Dict:
        return {
            'memory': self.get_memory_usage(),
            'pools': self.get_pool_stats(),
            'cleanups': self._cleanup_count,
            'cycles': self._cycle_count,
            'snapshots': len(self._snapshots),
        }


memory_manager = MemoryManager()
