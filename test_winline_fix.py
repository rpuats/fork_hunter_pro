import asyncio
import sys
import os
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
os.environ.setdefault('PYTHONIOENCODING', 'utf-8')
from scanner.parsers import WinlinePlaywrightParser

async def test():
    p = WinlinePlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'WINLINE: {len(events)} events')
    for e in events[:5]:
        home = e.get('home_team', 'EMPTY')
        away = e.get('away_team', 'EMPTY')
        # Encode to avoid Windows console issues
        print(f'  {home.encode("utf-8", errors="replace").decode()} vs {away.encode("utf-8", errors="replace").decode()}')
    
    # Check if team names are real (not Event_XXX fallbacks)
    real_names = 0
    fallback_names = 0
    for e in events:
        home = e.get('home_team', '')
        if home.startswith('Event_'):
            fallback_names += 1
        else:
            real_names += 1
    
    print(f'\nReal team names: {real_names}')
    print(f'Fallback names: {fallback_names}')
    print(f'SUCCESS: {real_names > fallback_names}')

asyncio.run(test())
