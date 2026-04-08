import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import BaltbetPlaywrightParser

async def scan():
    p = BaltbetPlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'BALTBET: {len(events)} events found')

asyncio.run(scan())
