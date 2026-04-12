import asyncio
from scanner.parsers.betcity_parser import BetcityParser

async def test():
    p = BetcityParser()
    events = await p.get_events()
    print(f'Betcity: {len(events)} events')

asyncio.run(test())