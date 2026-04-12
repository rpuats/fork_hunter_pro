import asyncio
import sys

sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')

from core.finder import SurebetCalculator
from scanner.parsers import (
    WinlinePlaywrightParser,
    PariPlaywrightParser,
    ZenitPlaywrightParser,
    BaltbetRegexParser,
)


async def find():
    calc = SurebetCalculator(min_profit=0.1)
    all_events = []

    parsers = [
        ('Winline', WinlinePlaywrightParser),
        ('Pari', PariPlaywrightParser),
        ('Zenit', ZenitPlaywrightParser),
        ('Baltbet', BaltbetRegexParser),
    ]

    for name, cls in parsers:
        try:
            p = cls()
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            all_events.extend(events)
            print(f'{name}: {len(events)} events')
        except Exception as e:
            print(f'{name}: FAILED - {e}')

    surebets = calc.find_3way_surebets(all_events)
    print(f'\n3-WAY SUREBETS (min 0.1%): {len(surebets)}')
    for sb in surebets[:10]:
        print(f'  {sb["event_name"]}: {sb["profit_percent"]:.2f}%')


asyncio.run(find())
