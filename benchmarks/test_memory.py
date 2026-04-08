# benchmarks/test_memory.py
"""Benchmark memory usage and leak detection."""
import gc
import sys
import time
import tracemalloc
from typing import Dict, List


def _generate_events(count: int) -> List[Dict]:
    events = []
    teams = [
        ('Реал Мадрид', 'Барселона'),
        ('Манчестер Юнайтед', 'Ливерпуль'),
        ('Бавария', 'Боруссия Дортмунд'),
        ('ПСЖ', 'Марсель'),
        ('Ювентус', 'Интер'),
        ('Зенит', 'Спартак Москва'),
    ]
    bookmakers = ['winline', 'olimp', 'pari', 'marathon', 'betboom', 'fonbet']

    for i in range(count):
        home, away = teams[i % len(teams)]
        bk = bookmakers[i % len(bookmakers)]
        events.append({
            'id': f'evt_{i}',
            'home_team': home,
            'away_team': away,
            'bookmaker': bk,
            'sport': 'football',
            'home_odds': round(1.5 + (i % 20) * 0.1, 2),
            'draw_odds': round(3.0 + (i % 10) * 0.1, 2),
            'away_odds': round(2.0 + (i % 15) * 0.1, 2),
            'is_live': True,
            'market': '1x2',
        })
    return events


def bench_event_pool_memory(events: List[Dict], cycles: int = 100) -> Dict:
    from core.event_pool import EventPool

    tracemalloc.start()
    pool = EventPool(max_size=10000)

    initial_mem = tracemalloc.get_traced_memory()[0]

    for _ in range(cycles):
        pool.upsert_batch(events)

    final_mem = tracemalloc.get_traced_memory()[0]
    peak_mem = tracemalloc.get_traced_memory()[1]

    tracemalloc.stop()

    return {
        'events': len(events),
        'cycles': cycles,
        'initial_kb': round(initial_mem / 1024, 1),
        'final_kb': round(final_mem / 1024, 1),
        'peak_kb': round(peak_mem / 1024, 1),
        'growth_kb': round((final_mem - initial_mem) / 1024, 1),
        'pool_size': pool.get_count(),
        'pool_stats': pool.stats(),
    }


def bench_finder_memory(events: List[Dict], cycles: int = 100) -> Dict:
    from core.finder import SurebetCalculator as OldCalculator
    from core.finder_optimized import OptimizedSurebetCalculator

    tracemalloc.start()

    old_calc = OldCalculator(min_profit=0.5)
    new_calc = OptimizedSurebetCalculator(min_profit=0.5)

    gc.collect()
    old_initial = tracemalloc.get_traced_memory()[0]
    for _ in range(cycles):
        old_calc.find_surebets(events)
    old_final = tracemalloc.get_traced_memory()[0]
    old_peak = tracemalloc.get_traced_memory()[1]

    gc.collect()
    new_initial = tracemalloc.get_traced_memory()[0]
    for _ in range(cycles):
        new_calc.find_surebets(events)
    new_final = tracemalloc.get_traced_memory()[0]
    new_peak = tracemalloc.get_traced_memory()[1]

    tracemalloc.stop()

    return {
        'events': len(events),
        'cycles': cycles,
        'old': {
            'growth_kb': round((old_final - old_initial) / 1024, 1),
            'peak_kb': round(old_peak / 1024, 1),
        },
        'new': {
            'growth_kb': round((new_final - new_initial) / 1024, 1),
            'peak_kb': round(new_peak / 1024, 1),
        },
    }


def bench_object_pool() -> Dict:
    from core.memory_manager import ObjectPool

    def dict_factory():
        return {'id': 0, 'data': None}

    pool = ObjectPool(factory=dict_factory, max_size=500, name="test_dict")

    start = time.perf_counter()
    for _ in range(10000):
        obj = pool.acquire()
        obj['id'] += 1
        pool.release(obj)
    elapsed = time.perf_counter() - start

    return {
        'operations': 10000,
        'time_ms': round(elapsed * 1000, 2),
        'ops_per_second': round(10000 / elapsed),
        'pool_stats': pool.stats(),
    }


if __name__ == '__main__':
    print("=== Memory Benchmark ===")
    events = _generate_events(500)

    print("\n--- EventPool Memory ---")
    r1 = bench_event_pool_memory(events, cycles=100)
    print(f"  Growth: {r1['growth_kb']:.1f}KB, Peak: {r1['peak_kb']:.1f}KB, Pool size: {r1['pool_size']}")

    print("\n--- Finder Memory ---")
    r2 = bench_finder_memory(events, cycles=50)
    print(f"  Old: growth={r2['old']['growth_kb']:.1f}KB, peak={r2['old']['peak_kb']:.1f}KB")
    print(f"  New: growth={r2['new']['growth_kb']:.1f}KB, peak={r2['new']['peak_kb']:.1f}KB")

    print("\n--- Object Pool ---")
    r3 = bench_object_pool()
    print(f"  {r3['ops_per_second']} ops/sec, reuse rate: {r3['pool_stats']['reuse_rate']}%")
