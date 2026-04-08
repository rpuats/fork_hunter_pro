import asyncio, sys, time
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers.pari_playwright import PariPlaywrightParser

async def test():
    t0 = time.time()
    p = PariPlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=60)
    print(f'PARI: {len(events)} events in {time.time()-t0:.1f}s')

asyncio.run(test())
