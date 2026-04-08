# core/performance.py
import time
import asyncio
import logging
import tracemalloc
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, field
from collections import defaultdict

logger = logging.getLogger(__name__)


@dataclass
class MetricPoint:
    timestamp: float
    value: float
    label: str = ""


@dataclass
class BookmakerMetrics:
    parse_times: List[float] = field(default_factory=list)
    event_counts: List[int] = field(default_factory=list)
    error_count: int = 0
    last_parse_time: float = 0.0
    avg_parse_time: float = 0.0
    events_per_second: float = 0.0


class PerformanceMonitor:
    """
    High-performance metrics collector for the scanner engine.
    Tracks parse times, surebet detection, memory usage, and throughput.
    """

    def __init__(self, max_history: int = 500):
        self.max_history = max_history
        self._bookmaker_metrics: Dict[str, BookmakerMetrics] = defaultdict(BookmakerMetrics)
        self._surebet_detection_times: List[float] = []
        self._cycle_times: List[float] = []
        self._events_per_cycle: List[int] = []
        self._memory_snapshots: List[Dict] = []
        self._start_time: float = time.time()
        self._total_events_processed: int = 0
        self._total_surebets_found: int = 0
        self._cache_hits: int = 0
        self._cache_misses: int = 0
        self._tracemalloc_started = False

    def start_tracemalloc(self):
        if not self._tracemalloc_started:
            tracemalloc.start()
            self._tracemalloc_started = True

    def stop_tracemalloc(self):
        if self._tracemalloc_started:
            tracemalloc.stop()
            self._tracemalloc_started = False

    def get_memory_usage(self) -> Dict:
        if self._tracemalloc_started:
            current, peak = tracemalloc.get_traced_memory()
            return {
                "current_mb": round(current / 1024 / 1024, 2),
                "peak_mb": round(peak / 1024 / 1024, 2),
            }
        return {"current_mb": 0, "peak_mb": 0}

    def record_parse_start(self, bookmaker: str) -> float:
        return time.monotonic()

    def record_parse_end(self, bookmaker: str, start_time: float, event_count: int, had_error: bool = False):
        elapsed = time.monotonic() - start_time
        bm = self._bookmaker_metrics[bookmaker]
        bm.parse_times.append(elapsed)
        bm.event_counts.append(event_count)
        bm.last_parse_time = elapsed

        if had_error:
            bm.error_count += 1

        if len(bm.parse_times) > self.max_history:
            bm.parse_times.pop(0)
            bm.event_counts.pop(0)

        bm.avg_parse_time = sum(bm.parse_times) / len(bm.parse_times)
        total_events = sum(bm.event_counts)
        total_time = sum(bm.parse_times)
        bm.events_per_second = total_events / total_time if total_time > 0 else 0

    def record_surebet_detection(self, elapsed: float, surebets_found: int):
        self._surebet_detection_times.append(elapsed)
        self._total_surebets_found += surebets_found
        if len(self._surebet_detection_times) > self.max_history:
            self._surebet_detection_times.pop(0)

    def record_cycle(self, elapsed: float, event_count: int):
        self._cycle_times.append(elapsed)
        self._events_per_cycle.append(event_count)
        self._total_events_processed += event_count
        if len(self._cycle_times) > self.max_history:
            self._cycle_times.pop(0)
            self._events_per_cycle.pop(0)

    def record_cache_hit(self):
        self._cache_hits += 1

    def record_cache_miss(self):
        self._cache_misses += 1

    def get_bookmaker_stats(self) -> Dict[str, Dict]:
        result = {}
        for bk, bm in self._bookmaker_metrics.items():
            result[bk] = {
                "avg_parse_time_ms": round(bm.avg_parse_time * 1000, 2),
                "last_parse_time_ms": round(bm.last_parse_time * 1000, 2),
                "events_per_second": round(bm.events_per_second, 1),
                "error_count": bm.error_count,
                "total_parses": len(bm.parse_times),
                "total_events": sum(bm.event_counts),
            }
        return result

    def get_surebet_stats(self) -> Dict:
        times = self._surebet_detection_times
        return {
            "avg_detection_time_ms": round(sum(times) / len(times) * 1000, 2) if times else 0,
            "max_detection_time_ms": round(max(times) * 1000, 2) if times else 0,
            "min_detection_time_ms": round(min(times) * 1000, 2) if times else 0,
            "total_surebets_found": self._total_surebets_found,
            "detection_count": len(times),
        }

    def get_throughput(self) -> Dict:
        cycles = self._cycle_times
        events = self._events_per_cycle
        uptime = time.time() - self._start_time

        total_events = sum(events) if events else 0
        total_time = sum(cycles) if cycles else 0

        return {
            "events_per_second": round(total_events / total_time, 1) if total_time > 0 else 0,
            "cycles_per_second": round(len(cycles) / uptime, 2) if uptime > 0 else 0,
            "avg_cycle_time_ms": round(sum(cycles) / len(cycles) * 1000, 2) if cycles else 0,
            "total_events_processed": self._total_events_processed,
            "total_cycles": len(cycles),
            "uptime_seconds": round(uptime, 1),
        }

    def get_cache_stats(self) -> Dict:
        total = self._cache_hits + self._cache_misses
        return {
            "hits": self._cache_hits,
            "misses": self._cache_misses,
            "hit_rate": round(self._cache_hits / total * 100, 2) if total > 0 else 0,
        }

    def get_full_report(self) -> Dict:
        return {
            "memory": self.get_memory_usage(),
            "throughput": self.get_throughput(),
            "bookmakers": self.get_bookmaker_stats(),
            "surebet_detection": self.get_surebet_stats(),
            "cache": self.get_cache_stats(),
        }

    def log_summary(self, cycle_num: int):
        if cycle_num % 10 == 0:
            report = self.get_full_report()
            mem = report["memory"]
            tp = report["throughput"]
            sb = report["surebet_detection"]
            logger.info(
                f"⚡ Perf Report | Cycle #{cycle_num} | "
                f"EPS: {tp['events_per_second']} | "
                f"Avg cycle: {tp['avg_cycle_time_ms']}ms | "
                f"Mem: {mem['current_mb']}MB (peak: {mem['peak_mb']}MB) | "
                f"Surebets: {sb['total_surebets_found']} | "
                f"Cache: {report['cache']['hit_rate']}%"
            )
