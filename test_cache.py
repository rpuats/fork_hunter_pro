import asyncio
import sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.cache import event_cache, surebet_cache, rate_limiter, AsyncTTLCache

async def test():
    cache = AsyncTTLCache(maxsize=100, default_ttl=60)
    await cache.set('test', {'data': 'value'})
    val = await cache.get('test')
    print(f'CACHE CHECK: {"OK" if val else "FAIL"}')
    print(f'  event_cache: {"OK" if event_cache else "FAIL"}')
    print(f'  surebet_cache: {"OK" if surebet_cache else "FAIL"}')
    print(f'  rate_limiter: {"OK" if rate_limiter else "FAIL"}')

asyncio.run(test())
