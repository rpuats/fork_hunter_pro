import asyncio, sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'scanner', 'parsers'))
from baltbet_playwright import BaltbetRegexParser

async def s():
    async with BaltbetRegexParser() as p:
        events = await p.get_events()
        print(f'BALTBET: {len(events)} events')

asyncio.run(s())
