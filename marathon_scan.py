import asyncio
import sys

sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import MarathonPlaywrightParser

async def scan():
    p = MarathonPlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'MARATHON: {len(events)} events')

asyncio.run(scan())
