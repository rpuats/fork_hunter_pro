import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.odds_verifier import OddsVerifier
from core.finder import SurebetCalculator
from scanner.parsers import WinlinePlaywrightParser, PariPlaywrightParser

async def verify():
    verifier = OddsVerifier()
    calc = SurebetCalculator(min_profit=0.5)
    
    all_events = []
    for name, cls in [('Winline', WinlinePlaywrightParser), ('Pari', PariPlaywrightParser)]:
        try:
            p = cls()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            all_events.extend(events)
            print(f'{name}: {len(events)} events')
        except Exception as e:
            print(f'{name}: ERROR - {e}')
    
    surebets = calc.find_2way_surebets(all_events)
    print(f'\nSurebets found: {len(surebets)}')
    
    valid = [sb for sb in surebets if verifier.verify_surebet(sb, all_events).is_valid]
    expired = len(surebets) - len(valid)
    print(f'Valid: {len(valid)}, Expired: {expired}')
    for sb in valid[:5]:
        print(f'  ✅ {sb["event_name"]}: {sb["profit_percent"]:.2f}%')

asyncio.run(verify())
