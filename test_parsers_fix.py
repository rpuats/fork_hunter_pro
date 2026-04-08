import asyncio
import sys
sys.path.insert(0, '.')
from scanner.parsers.betcity_playwright import BetcityPlaywrightParser
from scanner.parsers.marathon_playwright import MarathonPlaywrightParser

async def test():
    for name, cls in [('Betcity', BetcityPlaywrightParser), ('Marathon', MarathonPlaywrightParser)]:
        print(f'Testing {name}...')
        p = cls()
        try:
            events = await asyncio.wait_for(p.get_events(), timeout=90)
            print(f'{name}: {len(events)} events')
            if events:
                for e in events[:3]:
                    ht = e.get('home_team', '?')
                    at = e.get('away_team', '?')
                    ho = e.get('home_odds', '?')
                    do = e.get('draw_odds', '?')
                    ao = e.get('away_odds', '?')
                    print(f'  {ht} vs {at}: {ho} - {do} - {ao}')
        except Exception as e:
            print(f'{name}: ERROR - {str(e)[:80]}')

asyncio.run(test())
