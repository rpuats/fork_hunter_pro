#!/usr/bin/env python3
"""
Quick Parser Diagnostic - Проверка всех парсеров
Быстрая проверка без полного запуска Playwright (если возможно)
"""

import asyncio
import json
from datetime import datetime
from pathlib import Path

# Пытаемся импортировать парсеры
PARSERS = {}

try:
    from scanner.parsers.winline_playwright import WinlinePlaywrightParser
    PARSERS['Winline'] = WinlinePlaywrightParser
except Exception as e:
    print(f"❌ Winline import failed: {e}")

try:
    from scanner.parsers.pari_playwright import PariPlaywrightParser
    PARSERS['Pari'] = PariPlaywrightParser
except Exception as e:
    print(f"❌ Pari import failed: {e}")

try:
    from scanner.parsers.betcity_playwright import BetcityPlaywrightParser
    PARSERS['Betcity'] = BetcityPlaywrightParser
except Exception as e:
    print(f"❌ Betcity import failed: {e}")

try:
    from scanner.parsers.marathon_playwright import MarathonPlaywrightParser
    PARSERS['Marathon'] = MarathonPlaywrightParser
except Exception as e:
    print(f"❌ Marathon import failed: {e}")

try:
    from scanner.parsers.zenit_parser import ZenitParser
    PARSERS['Zenit'] = ZenitParser
except Exception as e:
    print(f"❌ Zenit import failed: {e}")

try:
    from scanner.parsers.leon_api import LeonParser
    PARSERS['Leon'] = LeonParser
except Exception as e:
    print(f"❌ Leon import failed: {e}")

try:
    from scanner.parsers.baltbet_parser import BaltbetParser
    PARSERS['Baltbet'] = BaltbetParser
except Exception as e:
    print(f"❌ Baltbet import failed: {e}")

try:
    from scanner.parsers.bettery_api import BetteryParser
    PARSERS['Bettery'] = BetteryParser
except Exception as e:
    print(f"❌ Bettery import failed: {e}")

async def test_parser(name, parser_class):
    """Test a single parser"""
    try:
        print(f"\n⏳ Testing {name}...", end="", flush=True)
        
        parser = parser_class()
        
        # Try to get events
        start = datetime.now()
        events = await parser.get_events()
        duration = (datetime.now() - start).total_seconds()
        
        if events:
            status = "✅"
            msg = f"{len(events):4d} events in {duration:.1f}s"
        else:
            status = "⚠️"
            msg = f"0 events in {duration:.1f}s"
        
        print(f"\r{status} {name:15} | {msg}")
        return {
            'name': name,
            'status': 'OK' if events else 'NO_EVENTS',
            'count': len(events) if events else 0,
            'duration': duration,
        }
    except Exception as e:
        print(f"\r❌ {name:15} | ERROR: {str(e)[:40]}")
        return {
            'name': name,
            'status': 'ERROR',
            'count': 0,
            'error': str(e)[:100],
        }

async def main():
    print("\n" + "=" * 70)
    print("🔍 PARSER DIAGNOSTIC - All Parsers Test")
    print("=" * 70)
    print(f"Total parsers found: {len(PARSERS)}")
    print("=" * 70)
    
    if not PARSERS:
        print("❌ No parsers found! Check scanner.parsers module.")
        return
    
    # Test all parsers concurrently with timeout
    tasks = [test_parser(name, cls) for name, cls in PARSERS.items()]
    results = await asyncio.wait_for(asyncio.gather(*tasks), timeout=300)
    
    # Summary
    print("\n" + "=" * 70)
    print("SUMMARY")
    print("=" * 70)
    
    total_events = 0
    total_ok = 0
    total_error = 0
    total_no_events = 0
    
    for result in results:
        if result['status'] == 'OK':
            total_ok += 1
            total_events += result['count']
        elif result['status'] == 'ERROR':
            total_error += 1
        elif result['status'] == 'NO_EVENTS':
            total_no_events += 1
    
    print(f"✅ Working parsers:   {total_ok}/{len(PARSERS)}")
    print(f"⚠️  No events found:  {total_no_events}/{len(PARSERS)}")
    print(f"❌ Errors:           {total_error}/{len(PARSERS)}")
    print(f"\n📊 Total events found: {total_events}")
    print("=" * 70)
    
    # Detailed results
    print("\nDETAILED RESULTS:")
    for result in sorted(results, key=lambda x: x['count'], reverse=True):
        if result['status'] == 'OK':
            print(f"  ✅ {result['name']:15} | {result['count']:5d} events | {result['duration']:6.1f}s")
        elif result['status'] == 'ERROR':
            print(f"  ❌ {result['name']:15} | ERROR: {result.get('error', 'Unknown')}")
        else:
            print(f"  ⚠️  {result['name']:15} | 0 events")
    
    return total_events, total_ok

if __name__ == '__main__':
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n\n⏸️  Test interrupted by user")
    except Exception as e:
        print(f"\n❌ Test failed: {e}")
        import traceback
        traceback.print_exc()
