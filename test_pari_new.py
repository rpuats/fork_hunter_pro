import asyncio
import sys
sys.stdout.reconfigure(encoding='utf-8')
from scanner.parsers.pari_api import PariParser

async def test():
    p = PariParser()
    events = await p.get_events()
    print(f'Pari: {len(events)} events')
    for e in events[:5]:
        print(f'  {e["home_team"]} vs {e["away_team"]}: 1={e["home_odds"]}, X={e["draw_odds"]}, 2={e["away_odds"]}')

asyncio.run(test())
