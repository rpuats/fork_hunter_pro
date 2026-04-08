#!/usr/bin/env python3
"""Quick benchmark runner that avoids multiprocessing issues."""
import sys
import os
import time
import json
import tracemalloc
import gc
from datetime import datetime

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def generate_events(count):
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
    events = []
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


def bench_detection():
    from core.finder import SurebetCalculator as OldCalc
    from core.finder_optimized import OptimizedSurebetCalculator, HAS_NUMPY
    
    results = {}
    for count in [100, 500, 1000, 5000]:
        events = generate_events(count)
        old = OldCalc(min_profit=0.5)
        new = OptimizedSurebetCalculator(min_profit=0.5)
        
        iterations = 50 if count <= 1000 else 20
        
        old_times = []
        for _ in range(iterations):
            gc.collect()
            start = time.perf_counter()
            old.find_surebets(events)
            old_times.append(time.perf_counter() - start)
        
        new_times = []
        for _ in range(iterations):
            gc.collect()
            start = time.perf_counter()
            new.find_surebets(events)
            new_times.append(time.perf_counter() - start)
        
        old_avg = sum(old_times) / len(old_times)
        new_avg = sum(new_times) / len(new_times)
        speedup = old_avg / new_avg if new_avg > 0 else 0
        
        results[f'{count}'] = {
            'old_ms': round(old_avg * 1000, 3),
            'new_ms': round(new_avg * 1000, 3),
            'speedup': round(speedup, 2),
        }
        print(f"  Detection {count} events: old={old_avg*1000:.3f}ms, new={new_avg*1000:.3f}ms, speedup={speedup:.2f}x (numpy={HAS_NUMPY})")
    
    return results


def bench_event_pool():
    from core.event_pool import EventPool
    
    results = {}
    for count in [100, 500, 1000]:
        events = generate_events(count)
        pool = EventPool(max_size=10000)
        
        iterations = 50
        times = []
        for _ in range(iterations):
            start = time.perf_counter()
            pool.upsert_batch(events)
            times.append(time.perf_counter() - start)
        
        avg = sum(times) / len(times)
        results[f'{count}'] = {
            'batch_ms': round(avg * 1000, 3),
            'eps': round(count / avg),
            'pool_size': pool.get_count(),
        }
        print(f"  EventPool {count} events: {avg*1000:.3f}ms batch ({count/avg:.0f} eps)")
    
    return results


def bench_memory():
    from core.event_pool import EventPool
    from core.finder import SurebetCalculator as OldCalc
    from core.finder_optimized import OptimizedSurebetCalculator
    from core.memory_manager import ObjectPool
    
    events = generate_events(500)
    
    # EventPool memory
    tracemalloc.start()
    pool = EventPool(max_size=10000)
    initial = tracemalloc.get_traced_memory()[0]
    for _ in range(100):
        pool.upsert_batch(events)
    final = tracemalloc.get_traced_memory()[0]
    peak = tracemalloc.get_traced_memory()[1]
    tracemalloc.stop()
    
    ep_mem = {
        'growth_kb': round((final - initial) / 1024, 1),
        'peak_kb': round(peak / 1024, 1),
    }
    print(f"  EventPool memory: growth={ep_mem['growth_kb']:.1f}KB, peak={ep_mem['peak_kb']:.1f}KB")
    
    # Finder memory
    tracemalloc.start()
    old = OldCalc(min_profit=0.5)
    gc.collect()
    old_init = tracemalloc.get_traced_memory()[0]
    for _ in range(50):
        old.find_surebets(events)
    old_final = tracemalloc.get_traced_memory()[0]
    old_peak = tracemalloc.get_traced_memory()[1]
    tracemalloc.stop()
    
    tracemalloc.start()
    new = OptimizedSurebetCalculator(min_profit=0.5)
    gc.collect()
    new_init = tracemalloc.get_traced_memory()[0]
    for _ in range(50):
        new.find_surebets(events)
    new_final = tracemalloc.get_traced_memory()[0]
    new_peak = tracemalloc.get_traced_memory()[1]
    tracemalloc.stop()
    
    finder_mem = {
        'old': {'growth_kb': round((old_final - old_init) / 1024, 1), 'peak_kb': round(old_peak / 1024, 1)},
        'new': {'growth_kb': round((new_final - new_init) / 1024, 1), 'peak_kb': round(new_peak / 1024, 1)},
    }
    print(f"  Finder memory: old growth={finder_mem['old']['growth_kb']:.1f}KB, new growth={finder_mem['new']['growth_kb']:.1f}KB")
    
    # Object pool
    def dict_factory():
        return {'id': 0, 'data': None}
    obj_pool = ObjectPool(factory=dict_factory, max_size=500, name="test")
    start = time.perf_counter()
    for _ in range(10000):
        obj = obj_pool.acquire()
        obj['id'] += 1
        obj_pool.release(obj)
    elapsed = time.perf_counter() - start
    
    op = {
        'ops_sec': round(10000 / elapsed),
        'reuse_rate': obj_pool.stats()['reuse_rate'],
    }
    print(f"  ObjectPool: {op['ops_sec']} ops/sec, reuse={op['reuse_rate']:.1f}%")
    
    return {'event_pool': ep_mem, 'finder': finder_mem, 'object_pool': op}


def main():
    print("=" * 60)
    print("  GHOST IMPERIUM — PERFORMANCE BENCHMARKS")
    print(f"  {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 60)
    
    total_start = time.time()
    
    print("\n[DETECTION] SUREBET DETECTION")
    print("-" * 40)
    det = bench_detection()
    
    print("\n[POOL] EVENT POOL")
    print("-" * 40)
    ep = bench_event_pool()
    
    print("\n[MEMORY] MEMORY")
    print("-" * 40)
    mem = bench_memory()
    
    elapsed = time.time() - total_start
    
    print("\n" + "=" * 60)
    print("  SUMMARY")
    print("=" * 60)
    
    avg_speedup = sum(r['speedup'] for r in det.values()) / len(det)
    print(f"  Avg detection speedup: {avg_speedup:.2f}x")
    print(f"  EventPool throughput: {ep.get('1000', {}).get('eps', 0):.0f} eps")
    print(f"  Memory growth (pool): {mem['event_pool']['growth_kb']:.1f}KB / 100 cycles")
    print(f"  ObjectPool: {mem['object_pool']['ops_sec']} ops/sec")
    print(f"  Total time: {elapsed:.1f}s")
    print("=" * 60)
    
    report = {
        'date': datetime.now().isoformat(),
        'detection': det,
        'event_pool': ep,
        'memory': mem,
        'total_seconds': round(elapsed, 1),
    }
    
    report_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'report.json')
    with open(report_path, 'w') as f:
        json.dump(report, f, indent=2, default=str)
    print(f"\nReport saved to benchmarks/report.json")


if __name__ == '__main__':
    main()
