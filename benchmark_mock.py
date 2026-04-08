import asyncio
import time
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

os.environ["USE_MOCK_DATA"] = "true"

from scanner.engine import GhostScanner, ScannerConfig
from services.database import Database

async def benchmark():
    print("=" * 60)
    print("PARSER SPEED BENCHMARK (MOCK MODE)")
    print("=" * 60)
    
    db = Database(':memory:')
    await db.init()
    config = ScannerConfig(
        enabled_sources={'winline', 'pari', 'baltbet', 'zenit', 'bettery'},
        cycle_interval=60
    )
    scanner = GhostScanner(db, config)
    
    print(f"\nEnabled parsers: {config.enabled_sources}")
    print(f"Loaded parsers: {[p.name for p in scanner.parsers]}")
    print()
    
    # First cycle (cold start)
    print("--- CYCLE 1 (COLD START) ---")
    start = time.time()
    await scanner._run_cycle()
    elapsed1 = time.time() - start
    print(f"Cycle 1 time: {elapsed1:.2f}s")
    
    # Second cycle (warm)
    print("\n--- CYCLE 2 (WARM) ---")
    start = time.time()
    await scanner._run_cycle()
    elapsed2 = time.time() - start
    print(f"Cycle 2 time: {elapsed2:.2f}s")
    
    # Third cycle (warm)
    print("\n--- CYCLE 3 (WARM) ---")
    start = time.time()
    await scanner._run_cycle()
    elapsed3 = time.time() - start
    print(f"Cycle 3 time: {elapsed3:.2f}s")
    
    stats = scanner.get_stats()
    
    print("\n" + "=" * 60)
    print("RESULTS")
    print("=" * 60)
    print(f'Cycle 1 (cold): {elapsed1:.2f}s')
    print(f'Cycle 2 (warm): {elapsed2:.2f}s')
    print(f'Cycle 3 (warm): {elapsed3:.2f}s')
    print(f'Average (warm): {(elapsed2 + elapsed3) / 2:.2f}s')
    print(f'Target: < 15s')
    print(f'Cycle 2 meets target: {"YES" if elapsed2 < 15 else "NO"}')
    print(f'Cycle 3 meets target: {"YES" if elapsed3 < 15 else "NO"}')
    
    print(f'\nEvents: {stats["total_events"]}')
    print(f'Surebets: {stats["total_surebets"]}')
    print(f'Verified: {stats.get("verified_count", 0)}')
    print(f'Expired: {stats.get("expired_count", 0)}')
    
    print(f'\nParser breakdown:')
    for slug, pstats in stats["parsers"].items():
        events = pstats.get("events", 0)
        error = pstats.get("error", "")
        requests = pstats.get("requests", 0)
        errors = pstats.get("errors", 0)
        status = f"ERROR: {error}" if error else f"{events} events"
        print(f'  {slug}: {status} (reqs: {requests}, errs: {errors})')
    
    await scanner.stop()
    
    print("\n" + "=" * 60)
    if elapsed2 < 15 and elapsed3 < 15:
        print("RESULT: OPTIMIZATION SUCCESSFUL - target met!")
    else:
        print("RESULT: OPTIMIZATION FAILED - target not met")
    print("=" * 60)
    
    return {
        'cycle1': elapsed1,
        'cycle2': elapsed2,
        'cycle3': elapsed3,
        'avg_warm': (elapsed2 + elapsed3) / 2,
        'target_met': elapsed2 < 15 and elapsed3 < 15,
        'events': stats["total_events"],
        'surebets': stats["total_surebets"],
        'parsers': stats["parsers"]
    }

if __name__ == "__main__":
    result = asyncio.run(benchmark())
    print(f"\nFinal result: {result}")
