import sys
import os
import asyncio

# Add paths - project root is TWO levels up from this script
script_dir = os.path.dirname(os.path.abspath(__file__))
# script_dir = .../scanner/parsers
# project_root = .../ (fork_hunter_pro root)
project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
if project_root not in sys.path:
    sys.path.insert(0, project_root)
print(f"Project root: {project_root}", flush=True)
print(f"Sys path: {sys.path[:3]}", flush=True)

from winline_playwright import WinlinePlaywrightParser

async def test():
    print("Starting Winline test...", flush=True)
    async with WinlinePlaywrightParser() as p:
        p.urls = ['https://winline.ru/football']
        events = await p.get_events()
        print(f'Found {len(events)} events', flush=True)
        if events:
            for e in events[:3]:
                print(f"  {e.get('home_team')} vs {e.get('away_team')}: {e.get('home_odds')} / {e.get('draw_odds')} / {e.get('away_odds')}", flush=True)

if __name__ == '__main__':
    asyncio.run(test())
