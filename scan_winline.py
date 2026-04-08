import asyncio, sys, os
os.environ['PYTHONIOENCODING'] = 'utf-8'
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import WinlinePlaywrightParser

async def scan():
    p = WinlinePlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'WINLINE: {len(events)} events found')
    for e in events[:3]:
        home = e.get("home_team","?").encode('ascii', 'ignore').decode()
        away = e.get("away_team","?").encode('ascii', 'ignore').decode()
        home_odds = e.get("home_odds","?")
        draw_odds = e.get("draw_odds","?")
        away_odds = e.get("away_odds","?")
        print(f'  {home} vs {away} | {home_odds} - {draw_odds} - {away_odds}')

asyncio.run(scan())
