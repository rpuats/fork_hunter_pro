import asyncio
import sys
sys.path.insert(0, '.')
from scanner.parsers.winline_playwright import WinlinePlaywrightParser

async def test():
    async with WinlinePlaywrightParser() as p:
        events = await asyncio.wait_for(p.get_events(), timeout=90)
        print(f'Total events: {len(events)}')
        
        totals_events = [e for e in events if e.get('total_over') and len(e['total_over']) > 0]
        print(f'Events with totals: {len(totals_events)}')
        
        one_x2_events = [e for e in events if e.get('home_odds') is not None]
        print(f'Events with 1x2: {len(one_x2_events)}')
        
        if totals_events:
            print('\n=== Sample totals events ===')
            for e in totals_events[:3]:
                print(f"  {e['home_team'].encode('ascii', 'ignore').decode('ascii')} vs {e['away_team'].encode('ascii', 'ignore').decode('ascii')}")
                print(f"    total_over: {e['total_over']}")
                print(f"    total_under: {e['total_under']}")
                print(f"    total_line: {e.get('total_line')}")
                print(f"    total_lines: {e.get('total_lines')}")
                print()
        
        if one_x2_events:
            print('\n=== Sample 1x2 events ===')
            for e in one_x2_events[:3]:
                print(f"  {e['home_team'].encode('ascii', 'ignore').decode('ascii')} vs {e['away_team'].encode('ascii', 'ignore').decode('ascii')}")
                print(f"    home: {e['home_odds']}, draw: {e['draw_odds']}, away: {e['away_odds']}")
                print()

asyncio.run(test())
