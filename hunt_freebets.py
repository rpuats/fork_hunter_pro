import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.freebet_hunter import FreebetHunter
from core.finder import SurebetCalculator
from scanner.parsers import WinlinePlaywrightParser, PariPlaywrightParser

async def hunt():
    hunter = FreebetHunter(min_freebet_roi=1.0)
    calc = SurebetCalculator(min_profit=0.1)
    all_events = []
    for cls in [WinlinePlaywrightParser, PariPlaywrightParser]:
        try:
            p = cls()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            all_events.extend(events)
            print(f"Got {len(events)} events from {cls.__name__}")
        except Exception as e:
            print(f"Error with {cls.__name__}: {e}")
    print(f"Total events: {len(all_events)}")
    surebets = calc.find_2way_surebets(all_events)
    print(f"Surebets found: {len(surebets)}")
    freebet_surebets = hunter.find_freebet_surebets(surebets)
    print(f"FREEBET SUREBETS: {len(freebet_surebets)}")
    for fb in freebet_surebets[:5]:
        print(f"  ROI {fb['roi_with_freebet']:.1f}% ({fb['freebet_bookmaker']} {fb['freebet_amount']}rub)")

asyncio.run(hunt())
