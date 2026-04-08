import asyncio
import sys
import os
import json

os.environ['PYTHONIOENCODING'] = 'utf-8'
sys.stdout.reconfigure(encoding='utf-8')

from scanner.parsers.ligastavok_playwright import LigaStavokPlaywrightParser

async def main():
    parser = LigaStavokPlaywrightParser()
    events = await parser.get_events()
    
    print(f"\nFound {len(events)} events")
    
    if events:
        for event in events[:5]:
            print(f"\n{event['home_team']} vs {event['away_team']}")
            print(f"  1={event['home_odds']}, X={event['draw_odds']}, 2={event['away_odds']}")
            print(f"  Totals: {len(event.get('totals', []))}")
            print(f"  Handicaps: {len(event.get('handicaps', []))}")
        
        with open('ligastavok_test_output.json', 'w', encoding='utf-8') as f:
            json.dump(events, f, ensure_ascii=False, indent=2)
        print(f"\nSaved to ligastavok_test_output.json")

asyncio.run(main())
