import asyncio
import sys
sys.stdout.reconfigure(encoding='utf-8')

async def test():
    from scanner.parsers.sportbet_api import SportbetApiParser
    p = SportbetApiParser()
    events = await p.get_events()
    print(f'Sportbet: {len(events)} events')
    
    live = [e for e in events if e.get('is_live')]
    prematch = [e for e in events if not e.get('is_live')]
    print(f'  Live: {len(live)}')
    print(f'  Prematch: {len(prematch)}')
    
    for e in events[:5]:
        print(f'  {e["home_team"]} vs {e["away_team"]}: 1={e["home_odds"]}, X={e["draw_odds"]}, 2={e["away_odds"]}')

asyncio.run(test())
