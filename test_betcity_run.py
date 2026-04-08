import asyncio
import sys

sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import BetcityPlaywrightParser

async def main():
    p = BetcityPlaywrightParser()
    events = await asyncio.wait_for(p.get_events(), timeout=90)
    print(f'BETCITY: {len(events)} events')

if __name__ == '__main__':
    asyncio.run(main())
