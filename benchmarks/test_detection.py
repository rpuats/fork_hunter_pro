import time
import asyncio
from typing import Dict, List

from core.finder_optimized import OptimizedSurebetCalculator, ParallelSurebetDetector


def _generate_events(count: int) -> List[Dict]:
    events = []
    teams = [
        ('Реал Мадрид', 'Барселона'),
        ('Манчестер Юнайтед', 'Ливерпуль'),
        ('Бавария', 'Боруссия Дортмунд'),
        ('ПСЖ', 'Марсель'),
        ('Ювентус', 'Интер'),
        ('Зенит', 'Спартак Москва'),
        ('ЦСКА Москва', 'Локомотив Москва'),
        ('Краснодар', 'Ростов'),
    ]
    bookmakers = ['winline', 'olimp', 'pari', 'marathon', 'betboom', 'fonbet', '1xstavka', 'leon']

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


def bench_detection(events: List[Dict], iterations: int = 100) -> Dict:
    from core.finder import SurebetCalculator as OldCalculator
    from core.finder_optimized import OptimizedSurebetCalculator

    old_calc = OldCalculator(min_profit=0.5)
    new_calc = OptimizedSurebetCalculator(min_profit=0.5)

    old_times = []
    new_times = []

    for _ in range(iterations):
        start = time.perf_counter()
        old_calc.find_surebets(events)
        old_times.append(time.perf_counter() - start)

        start = time.perf_counter()
        new_calc.find_surebets(events)
        new_times.append(time.perf_counter() - start)

    old_avg = sum(old_times) / len(old_times)
    new_avg = sum(new_times) / len(new_times)
    speedup = old_avg / new_avg if new_avg > 0 else 0

    return {
        'events': len(events),
        'iterations': iterations,
        'old_avg_ms': round(old_avg * 1000, 3),
        'new_avg_ms': round(new_avg * 1000, 3),
        'speedup_x': round(speedup, 2),
        'numpy_enabled': new_calc._calc._hits >= 0,
    }


if __name__ == '__main__':
    print("=== Surebet Detection Benchmark ===")
    for count in [100, 500, 1000, 5000]:
        events = _generate_events(count)
        result = bench_detection(events, iterations=50)
        print(f"  {result['events']} events: old={result['old_avg_ms']:.3f}ms, new={result['new_avg_ms']:.3f}ms, speedup={result['speedup_x']:.2f}x")
