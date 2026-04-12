import asyncio
import sys
sys.path.insert(0, '.')

from scanner.parsers import (
    WinlinePlaywrightParser,
    PariPlaywrightParser,
    BaltbetRegexParser,
    BetteryPlaywrightParser,
    BetcityPlaywrightParser,
    MarathonPlaywrightParser,
    ZenitPlaywrightParser,
)

async def test():
    results = {}
    parsers = [
        ('Winline', WinlinePlaywrightParser()),
        ('Pari', PariPlaywrightParser()),
        ('Baltbet', BaltbetRegexParser()),
        ('Bettery', BetteryPlaywrightParser()),
        ('Betcity', BetcityPlaywrightParser()),
        ('Marathon', MarathonPlaywrightParser()),
        ('Zenit', ZenitPlaywrightParser()),
    ]
    for name, parser in parsers:
        try:
            print(f"Testing {name}...")
            events = await asyncio.wait_for(parser.get_events(), timeout=60)
            results[name] = {'status': 'OK', 'events': len(events)}
            print(f"  {name}: OK ({len(events)} events)")
        except Exception as e:
            results[name] = {'status': 'ERROR', 'error': str(e)[:80]}
            print(f"  {name}: ERROR ({str(e)[:80]})")
    return results

result = asyncio.run(test())
print("\n=== SUMMARY ===")
for name, res in result.items():
    if res['status'] == 'OK':
        print(f"- {name}: OK ({res['events']} events)")
    else:
        print(f"- {name}: ERROR ({res['error']})")
