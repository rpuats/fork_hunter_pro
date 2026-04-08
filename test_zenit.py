import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import ZenitPlaywrightParser

async def s():
    p = ZenitPlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'ZENIT: {len(events)} events')

asyncio.run(s())
