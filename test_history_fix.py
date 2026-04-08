import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.surebet_history import SurebetHistory

async def test():
    history = SurebetHistory()
    await history.init()
    sb = {
        'event_name': 'Test Match',
        'profit_percent': 2.5,
        'legs': [{'bookmaker': 'winline'}, {'bookmaker': 'pari'}],
        'status': 'active'
    }
    await history.save_surebet(sb)
    heatmap = await history.get_bookmaker_heatmap()
    print(f'HEATMAP: {heatmap}')
    await history.close()

asyncio.run(test())
