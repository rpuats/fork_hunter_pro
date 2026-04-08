import asyncio
import json
from scanner.parsers.winline_playwright import WinlinePlaywrightParser

async def test():
    parser = WinlinePlaywrightParser()
    async with parser:
        events = await parser.get_events()
        print(f'Total events: {len(events)}')
        
        # Save to JSON
        with open('winline_real_events.json', 'w', encoding='utf-8') as f:
            json.dump(events, f, ensure_ascii=False, indent=2)
        print('Saved to winline_real_events.json')
        
        # Count types
        two_way = [e for e in events if e['draw_odds'] is None]
        three_way = [e for e in events if e['draw_odds'] is not None]
        print(f'2-way: {len(two_way)}')
        print(f'3-way: {len(three_way)}')

asyncio.run(test())
