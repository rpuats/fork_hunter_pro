import asyncio
import sys
import os
import json
sys.path.insert(0, '.')
os.environ['PYTHONIOENCODING'] = 'utf-8'
from scanner.parsers.marathon_playwright import MarathonPlaywrightParser
import logging

logging.basicConfig(level=logging.DEBUG)

async def test_marathon():
    parser = MarathonPlaywrightParser()
    try:
        events = await asyncio.wait_for(parser.get_events(), timeout=90)
        print(f"Marathon: {len(events)} events")
        if events:
            for e in events[:3]:
                print(f"  {e.get('home_team', '?')} vs {e.get('away_team', '?')}: {e.get('home_odds', '?')} - {e.get('draw_odds', '?')} - {e.get('away_odds', '?')}")
        else:
            print("  No events found!")
    except Exception as e:
        print(f"Marathon: ERROR - {type(e).__name__}: {str(e)[:200]}")
        import traceback
        traceback.print_exc()

asyncio.run(test_marathon())
