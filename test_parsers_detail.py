import asyncio
import sys
import traceback
sys.path.insert(0, '.')

from scanner.parsers import (
    BaltbetPlaywrightParser,
    BetteryPlaywrightParser,
    BetcityPlaywrightParser,
    MarathonPlaywrightParser,
)

async def test_parser(name, parser):
    try:
        print(f"Testing {name}...")
        events = await asyncio.wait_for(parser.get_events(), timeout=60)
        print(f"  {name}: OK ({len(events)} events)")
        return {'status': 'OK', 'events': len(events)}
    except asyncio.TimeoutError:
        print(f"  {name}: TIMEOUT (60s)")
        return {'status': 'ERROR', 'error': 'Timeout 60s'}
    except Exception as e:
        tb = traceback.format_exc()
        print(f"  {name}: ERROR")
        print(f"  Full traceback:\n{tb}")
        return {'status': 'ERROR', 'error': str(e)[:150]}

async def main():
    parsers = [
        ('Baltbet', BaltbetPlaywrightParser()),
        ('Bettery', BetteryPlaywrightParser()),
        ('Betcity', BetcityPlaywrightParser()),
        ('Marathon', MarathonPlaywrightParser()),
    ]
    results = {}
    for name, parser in parsers:
        results[name] = await test_parser(name, parser)
    print("\n=== SUMMARY ===")
    for name, res in results.items():
        if res['status'] == 'OK':
            print(f"- {name}: OK ({res['events']} events)")
        else:
            print(f"- {name}: ERROR ({res['error']})")

asyncio.run(main())
