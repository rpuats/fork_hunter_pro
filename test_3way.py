import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.finder import SurebetCalculator
from scanner.parsers import WinlinePlaywrightParser, PariPlaywrightParser, ZenitPlaywrightParser, BaltbetPlaywrightParser

async def find():
    calc = SurebetCalculator(min_profit=0.1)
    all_events = []
    for name, cls in [('Winline', WinlinePlaywrightParser), ('Pari', PariPlaywrightParser), ('Zenit', ZenitPlaywrightParser), ('Baltbet', BaltbetPlaywrightParser)]:
        try:
            p = cls()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            all_events.extend(events)
        except: pass
    
    surebets = calc.find_3way_surebets(all_events)
    print(f'3-WAY SUREBETS (min 0.1%): {len(surebets)}')
    for sb in surebets[:10]:
        print(f'  {sb["event_name"]}: {sb["profit_percent"]:.2f}%')

asyncio.run(find())
