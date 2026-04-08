import asyncio, sys, os
os.environ['PYTHONIOENCODING'] = 'utf-8'
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import WinlinePlaywrightParser

async def s():
    p = WinlinePlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'WINLINE: {len(events)} events')
    for e in events[:3]:
        home = e.get("home_team","?")
        away = e.get("away_team","?")
        h_odds = e.get("home_odds")
        d_odds = e.get("draw_odds")
        a_odds = e.get("away_odds")
        print(f'  {home} vs {away} | {h_odds} - {d_odds} - {a_odds}')

asyncio.run(s())
