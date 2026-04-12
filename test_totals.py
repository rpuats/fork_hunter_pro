import asyncio
import logging

logging.basicConfig(level=logging.WARNING)

async def test():
    from scanner.parsers import ZenitPlaywrightParser, BaltbetRegexParser, BetcityPlaywrightParser
    for name, cls in [('Zenit', ZenitPlaywrightParser), ('Baltbet', BaltbetRegexParser), ('Betcity', BetcityPlaywrightParser)]:
        p = cls()
        try:
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            with_totals = [e for e in events if e.get('total_over')]
            print(f'{name}: {len(events)} events, {len(with_totals)} with totals')
            if with_totals:
                for e in with_totals[:3]:
                    print(f'  {e["home_team"]} vs {e["away_team"]}: OVER={e["total_over"]} UNDER={e["total_under"]}')
        except Exception as e:
            print(f'{name}: ERROR - {str(e)[:60]}')

asyncio.run(test())
