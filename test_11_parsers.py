import asyncio, sys, time
sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from scanner.parsers import (
    WinlinePlaywrightParser, PariPlaywrightParser, BetcityPlaywrightParser,
    ZenitPlaywrightParser, BaltbetRegexParser, MarathonPlaywrightParser,
    BetteryPlaywrightParser, BetBoomPlaywrightParser, FonbetPlaywrightParser,
    _24betPlaywrightParser, OlimpBetPlaywrightParser
)

async def test_one(name, cls):
    t0 = time.time()
    try:
        p = cls()
        events = await asyncio.wait_for(p.get_events(), timeout=120)
        elapsed = time.time() - t0
        print('{}: {} events in {:.1f}s'.format(name, len(events), elapsed))
        return len(events)
    except Exception as e:
        elapsed = time.time() - t0
        print('{}: ERROR in {:.1f}s - {}'.format(name, elapsed, e))
        return 0

async def main():
    parsers = [
        ('Winline', WinlinePlaywrightParser),
        ('Betcity', BetcityPlaywrightParser),
        ('Zenit', ZenitPlaywrightParser),
        ('Baltbet', BaltbetRegexParser),
        ('Marathon', MarathonPlaywrightParser),
        ('Bettery', BetteryPlaywrightParser),
        ('BetBoom', BetBoomPlaywrightParser),
        ('Fonbet', FonbetPlaywrightParser),
        ('24bet', _24betPlaywrightParser),
        ('OlimpBet', OlimpBetPlaywrightParser),
        ('Pari', PariPlaywrightParser),
    ]
    total = 0
    for name, cls in parsers:
        count = await test_one(name, cls)
        total += count
    print('')
    print('TOTAL: {} events from 11 BKs'.format(total))

asyncio.run(main())
