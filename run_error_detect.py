import asyncio, sys
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.odds_error_detector import OddsErrorDetector
from scanner.parsers import WinlinePlaywrightParser, PariPlaywrightParser, ZenitPlaywrightParser

async def detect():
    detector = OddsErrorDetector()
    all_events = []
    for name, cls in [('Winline', WinlinePlaywrightParser), ('Pari', PariPlaywrightParser), ('Zenit', ZenitPlaywrightParser)]:
        try:
            p = cls()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            all_events.extend(events)
            print(f'{name}: {len(events)} events')
        except Exception as e:
            print(f'{name}: ERROR - {e}')
    
    errors = detector.get_errors(all_events)
    print(f'\nODDS ERRORS FOUND: {len(errors)}')
    for err in errors[:5]:
        bm = err.get('bookmaker', '?')
        ht = err.get('home_team', '?')
        at = err.get('away_team', '?')
        sc = err.get('error_score', 0)
        print(f'  {bm}: {ht} vs {at} - score {sc:.1f}')

asyncio.run(detect())
