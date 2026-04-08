# benchmarks/test_parsing.py
"""Benchmark parser speed and event processing."""
import time
import asyncio
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


def bench_event_pool(events: List[Dict], iterations: int = 50) -> Dict:
    from core.event_pool import EventPool

    pool = EventPool(max_size=10000)

    total_time = 0
    for _ in range(iterations):
        start = time.perf_counter()
        pool.upsert_batch(events)
        total_time += time.perf_counter() - start

    avg_ms = (total_time / iterations) * 1000
    return {
        'events': len(events),
        'iterations': iterations,
        'avg_batch_upsert_ms': round(avg_ms, 3),
        'events_per_second': round(len(events) / (avg_ms / 1000)),
        'pool_stats': pool.stats(),
    }


def bench_normalizer(events: List[Dict], iterations: int = 50) -> Dict:
    from core.normalizer import event_normalizer

    total_time = 0
    for _ in range(iterations):
        start = time.perf_counter()
        for e in events:
            event_normalizer.normalize_event(e['home_team'], e['away_team'])
        total_time += time.perf_counter() - start

    avg_ms = (total_time / iterations) * 1000
    return {
        'events': len(events),
        'iterations': iterations,
        'avg_normalize_ms': round(avg_ms, 3),
        'events_per_second': round(len(events) / (avg_ms / 1000)),
    }


def bench_event_grouping(events: List[Dict], iterations: int = 50) -> Dict:
    from core.finder import SurebetCalculator as OldCalculator
    from core.finder_optimized import OptimizedSurebetCalculator

    old_calc = OldCalculator(min_profit=0.5)
    new_calc = OptimizedSurebetCalculator(min_profit=0.5)

    old_times = []
    new_times = []

    for _ in range(iterations):
        start = time.perf_counter()
        old_calc._group_events_optimized(events)
        old_times.append(time.perf_counter() - start)

        start = time.perf_counter()
        new_calc._group_events(events)
        new_times.append(time.perf_counter() - start)

    old_avg = sum(old_times) / len(old_times)
    new_avg = sum(new_times) / len(new_times)

    return {
        'events': len(events),
        'old_avg_ms': round(old_avg * 1000, 3),
        'new_avg_ms': round(new_avg * 1000, 3),
        'speedup_x': round(old_avg / new_avg if new_avg > 0 else 0, 2),
    }


if __name__ == '__main__':
    print("=== Parsing & Event Processing Benchmark ===")
    for count in [100, 500, 1000]:
        events = _generate_events(count)
        print(f"\n--- {count} events ---")
        r1 = bench_event_pool(events, iterations=50)
        print(f"  EventPool upsert: {r1['avg_batch_upsert_ms']:.3f}ms ({r1['events_per_second']:.0f} eps)")
        r2 = bench_normalizer(events, iterations=50)
        print(f"  Normalizer: {r2['avg_normalize_ms']:.3f}ms ({r2['events_per_second']:.0f} eps)")
        r3 = bench_event_grouping(events, iterations=50)
        print(f"  Grouping: old={r3['old_avg_ms']:.3f}ms, new={r3['new_avg_ms']:.3f}ms, speedup={r3['speedup_x']:.2f}x")
