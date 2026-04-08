import asyncio
import sys
import time
sys.stdout.reconfigure(encoding='utf-8')

async def test_24bet():
    print("=== Testing 24bet ===")
    from scanner.parsers._24bet_playwright import _24betPlaywrightParser
    t0 = time.time()
    p = _24betPlaywrightParser()
    events = await p.get_events()
    print(f"24bet: {len(events)} events ({time.time()-t0:.1f}s)")
    for e in events[:3]:
        print(f"  {e['home_team']} vs {e['away_team']}: 1={e['home_odds']}, X={e.get('draw_odds')}, 2={e['away_odds']}")
    return events

async def test_sportbet():
    print("\n=== Testing Sportbet ===")
    from scanner.parsers.sportbet_playwright import SportbetPlaywrightParser
    t0 = time.time()
    p = SportbetPlaywrightParser()
    events = await p.get_events()
    print(f"Sportbet: {len(events)} events ({time.time()-t0:.1f}s)")
    for e in events[:3]:
        print(f"  {e['home_team']} vs {e['away_team']}: 1={e['home_odds']}, X={e.get('draw_odds')}, 2={e['away_odds']}")
    return events

async def main():
    e1 = await test_24bet()
    e2 = await test_sportbet()
    print(f"\nTotal: {len(e1) + len(e2)} events")

asyncio.run(main())
