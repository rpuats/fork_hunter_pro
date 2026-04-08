import asyncio
from scanner.parsers import BetcityPlaywrightParser

async def t():
    p = BetcityPlaywrightParser()
    ev = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'Betcity:{len(ev)} events')

asyncio.run(t())
