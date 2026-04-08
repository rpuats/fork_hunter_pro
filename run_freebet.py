import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.freebet_hunter import FreebetHunter
from core.finder import SurebetCalculator
from scanner.parsers import WinlinePlaywrightParser, PariPlaywrightParser, ZenitPlaywrightParser

async def hunt():
    hunter = FreebetHunter(min_freebet_roi=1.0)
    calc = SurebetCalculator(min_profit=0.1)
    
    all_events = []
    for name, cls in [('Winline', WinlinePlaywrightParser), ('Pari', PariPlaywrightParser), ('Zenit', ZenitPlaywrightParser)]:
        try:
            p = cls()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            all_events.extend(events)
            print(f'{name}: {len(events)} events')
        except Exception as e:
            print(f'{name}: ERROR - {e}')
    
    surebets = calc.find_2way_surebets(all_events)
    print(f'\nSurebets found: {len(surebets)}')
    
    freebet_surebets = hunter.find_freebet_surebets(surebets)
    print(f'Freebet surebets: {len(freebet_surebets)}')
    for fb in freebet_surebets[:5]:
        ev = fb['original_surebet']['event_name']
        roi = fb['roi_with_freebet']
        bk = fb['freebet_bookmaker']
        amt = fb['freebet_amount']
        print(f'  {ev}: ROI {roi:.1f}% (freebet: {bk} {amt} RUB)')

asyncio.run(hunt())
