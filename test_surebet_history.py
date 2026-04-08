import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.surebet_history import SurebetHistory

async def test():
    history = SurebetHistory()
    await history.init()
    
    for i in range(5):
        sb = {
            'event_name': f'Match {i+1}',
            'profit_percent': 1.5 + i * 0.5,
            'legs': [{'bookmaker': 'winline'}, {'bookmaker': 'pari'}],
            'status': 'active'
        }
        await history.save_surebet(sb)
    
    stats = await history.get_surebet_stats()
    print(f'SUREBET HISTORY: {stats.get("total", 0)} surebets stored')
    print(f'  Avg profit: {stats.get("avg_profit", 0):.2f}%')
    
    heatmap = await history.get_bookmaker_heatmap()
    print(f'  BK Heatmap: {heatmap}')
    
    await history.close()

asyncio.run(test())
