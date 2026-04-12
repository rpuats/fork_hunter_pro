import asyncio
from scanner.parsers import BetcityParser

async def t():
    p = BetcityParser()
    ev = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'Betcity:{len(ev)} events')

asyncio.run(t())
