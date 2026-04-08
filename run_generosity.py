import asyncio
import sys

sys.path.insert(0, r'C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro')
from core.generosity_index import BookmakerGenerosityIndex
from scanner.parsers.winline_playwright import WinlinePlaywrightParser
from scanner.parsers.pari_playwright import PariPlaywrightParser
from scanner.parsers.zenit_playwright import ZenitPlaywrightParser
from scanner.parsers.baltbet_playwright import BaltbetPlaywrightParser

async def calc():
    gi = BookmakerGenerosityIndex()
    all_events = []
    
    parsers = [
        WinlinePlaywrightParser(),
        PariPlaywrightParser(),
        ZenitPlaywrightParser(),
        BaltbetPlaywrightParser(),
    ]
    
    for p in parsers:
        try:
            print(f"Scanning {type(p).__name__}...")
            events = await asyncio.wait_for(p.get_events(), timeout=60)
            print(f"  Got {len(events)} events")
            all_events.extend(events)
        except Exception as e:
            print(f"  Error: {e}")
    
    print(f"\nTotal events: {len(all_events)}")
    gi.calculate_index(all_events)
    
    ranking = gi.get_ranking()
    print('\nGENEROSITY RANKING:')
    for bk, score in ranking:
        print(f'  {bk}: {score:.3f}')
    
    summary = gi.get_summary()
    print(f'\nSUMMARY: {summary}')

asyncio.run(calc())
