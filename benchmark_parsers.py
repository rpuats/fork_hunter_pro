# benchmark_parsers.py - Parser Performance Benchmark
# Run: python benchmark_parsers.py
"""
Measures each parser's execution time individually and in parallel.
"""
import asyncio
import time
import sys
import os

sys.path.insert(0, os.path.dirname(__file__))

PARSER_CONFIG = {
    'winline': ('scanner.parsers.winline_playwright', 'WinlinePlaywrightParser'),
    'pari': ('scanner.parsers.pari_playwright', 'PariPlaywrightParser'),
    'betcity': ('scanner.parsers.betcity_playwright', 'BetcityPlaywrightParser'),
    'marathon': ('scanner.parsers.marathon_playwright', 'MarathonPlaywrightParser'),
    'zenit': ('scanner.parsers.zenit_playwright', 'ZenitPlaywrightParser'),
    'bettery': ('scanner.parsers.bettery_playwright', 'BetteryPlaywrightParser'),
    'baltbet': ('scanner.parsers.baltbet_playwright', 'BaltbetPlaywrightParser'),
}


async def benchmark_parser(slug: str, module_name: str, class_name: str) -> dict:
    """Benchmark a single parser"""
    try:
        module = __import__(module_name, fromlist=[class_name])
        cls = getattr(module, class_name)
        parser = cls()
        
        start = time.monotonic()
        events = await parser.get_events()
        elapsed = time.monotonic() - start
        
        try:
            await parser.close()
        except:
            pass
        
        return {
            'slug': slug,
            'status': 'OK',
            'events': len(events),
            'time': elapsed,
            'error': None
        }
    except Exception as e:
        return {
            'slug': slug,
            'status': 'ERROR',
            'events': 0,
            'time': 0,
            'error': str(e)
        }


async def run_sequential():
    """Run parsers sequentially (OLD behavior)"""
    print("\n" + "="*60)
    print("SEQUENTIAL BENCHMARK (OLD behavior)")
    print("="*60)
    
    total_start = time.monotonic()
    results = []
    
    for slug, (module_name, class_name) in PARSER_CONFIG.items():
        print(f"  Running {slug}...")
        result = await benchmark_parser(slug, module_name, class_name)
        results.append(result)
        status_icon = "OK" if result['status'] == 'OK' else "ERR"
        print(f"    {slug}: {result['events']} events in {result['time']:.1f}s [{status_icon}]")
    
    total_time = time.monotonic() - total_start
    return results, total_time


async def run_parallel():
    """Run parsers in parallel (NEW behavior)"""
    print("\n" + "="*60)
    print("PARALLEL BENCHMARK (NEW behavior)")
    print("="*60)
    
    total_start = time.monotonic()
    
    tasks = []
    for slug, (module_name, class_name) in PARSER_CONFIG.items():
        tasks.append(benchmark_parser(slug, module_name, class_name))
    
    results = await asyncio.gather(*tasks, return_exceptions=True)
    
    processed = []
    for r in results:
        if isinstance(r, Exception):
            processed.append({'slug': '?', 'status': 'ERROR', 'events': 0, 'time': 0, 'error': str(r)})
        else:
            processed.append(r)
        status_icon = "OK" if r.get('status') == 'OK' else "ERR"
        print(f"    {r.get('slug', '?')}: {r.get('events', 0)} events in {r.get('time', 0):.1f}s [{status_icon}]")
    
    total_time = time.monotonic() - total_start
    return processed, total_time


async def main():
    print("Ghost Imperium - Parser Benchmark")
    print(f"Testing {len(PARSER_CONFIG)} parsers...")
    
    # Sequential benchmark
    seq_results, seq_time = await run_sequential()
    
    # Parallel benchmark
    par_results, par_time = await run_parallel()
    
    # Summary
    print("\n" + "="*60)
    print("SUMMARY")
    print("="*60)
    print(f"Sequential total: {seq_time:.1f}s")
    print(f"Parallel total:   {par_time:.1f}s")
    print(f"Speedup:          {seq_time/par_time:.1f}x")
    print(f"Target:           <15s")
    print(f"Status:           {'PASS' if par_time < 15 else 'FAIL'}")
    
    # Per-parser breakdown
    print("\nPer-parser times:")
    for r in seq_results:
        print(f"  {r['slug']:12s}: {r['time']:5.1f}s | {r['events']:3d} events | {r['status']}")
    
    return par_time < 15


if __name__ == '__main__':
    success = asyncio.run(main())
    sys.exit(0 if success else 1)
