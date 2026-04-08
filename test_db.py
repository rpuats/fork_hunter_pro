import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from services.database import Database

async def test():
    db = Database(':memory:')
    await db.init()
    print(f'DATABASE CHECK: {"OK" if db else "FAIL"}')

asyncio.run(test())
