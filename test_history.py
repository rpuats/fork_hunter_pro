import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.surebet_history import SurebetHistory

async def test():
    history = SurebetHistory()
    await history.init()
    
    for i in range(3):
        sb = {
            'event_name': f'Match {i+1}',
            'profit_percent': 1.5 + i * 0.5,
            'legs': [{'bookmaker': 'winline'}, {'bookmaker': 'pari'}],
            'status': 'active'
        }
        await history.save_surebet(sb)
    
    stats = await history.get_surebet_stats()
    total = stats.get('total', 0)
    avg = stats.get('avg_profit', 0)
    print(f'SUREBET HISTORY: {total} surebets stored')
    print(f'  Avg profit: {avg:.2f}%')
    
    heatmap = await history.get_bookmaker_heatmap()
    print(f'  BK Heatmap: {heatmap}')

asyncio.run(test())
