import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')

from scanner.parsers.zenit_parser import ZenitParser
import asyncio

async def test():
    parser = ZenitParser()
    events = await parser.get_events()
    print(f"Total events: {len(events)}")

    # Count by sport
    sport_counts = {}
    for event in events:
        sport = event.get('sport', 'unknown')
        sport_counts[sport] = sport_counts.get(sport, 0) + 1

    for sport, count in sport_counts.items():
        print(f"{sport}: {count} events")

asyncio.run(test())