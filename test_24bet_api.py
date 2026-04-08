import asyncio
import sys
sys.stdout.reconfigure(encoding='utf-8')

async def test():
    from scanner.parsers._24bet_api import _24betApiParser
    p = _24betApiParser()
    events = await p.get_events()
    print(f'24bet: {len(events)} events (football with 1X2)')
    
    live = [e for e in events if e.get('is_live')]
    prematch = [e for e in events if not e.get('is_live')]
    print(f'  Live: {len(live)}')
    print(f'  Prematch: {len(prematch)}')
    
    for e in events[:5]:
        print(f'  {e["home_team"]} vs {e["away_team"]}: 1={e["home_odds"]}, X={e["draw_odds"]}, 2={e["away_odds"]}')

asyncio.run(test())
