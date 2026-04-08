import asyncio
import sys
import io

if sys.platform == 'win32':
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

import logging
logging.basicConfig(level=logging.WARNING)

from scanner.parsers import (
    WinlinePlaywrightParser,
    PariPlaywrightParser, 
    BetcityPlaywrightParser,
    MarathonPlaywrightParser,
    ZenitPlaywrightParser,
)


async def scan_all():
    print("\n" + "=" * 60)
    print("SCANNING ALL BOOKMAKERS")
    print("=" * 60)
    
    parsers = [
        ('Winline', WinlinePlaywrightParser()),
        ('Pari', PariPlaywrightParser()),
        ('Betcity', BetcityPlaywrightParser()),
        ('Marathon', MarathonPlaywrightParser()),
        ('Zenit', ZenitPlaywrightParser()),
    ]
    
    all_events = []
    
    for name, parser in parsers:
        print(f"\nScanning {name}...", end=" ", flush=True)
        events = await parser.get_events()
        print(f"{len(events)} events")
        all_events.extend(events)
    
    print("\n" + "=" * 60)
    print(f"TOTAL EVENTS: {len(all_events)}")
    print("=" * 60)
    
    # Count by bookmaker
    by_bk = {}
    for e in all_events:
        bk = e.get('bookmaker', 'unknown')
        by_bk[bk] = by_bk.get(bk, 0) + 1
    
    print("\nBy bookmaker:")
    for bk, count in sorted(by_bk.items(), key=lambda x: -x[1]):
        print(f"  {bk}: {count}")
    
    # Now find surebets
    print("\n" + "=" * 60)
    print("SEARCHING FOR SUREBETS")
    print("=" * 60)
    
    from core.finder import SurebetCalculator
    
    calculator = SurebetCalculator()
    two_way = calculator.find_2way_surebets(all_events)
    three_way = calculator.find_3way_surebets(all_events)
    surebets = two_way + three_way
    
    print(f"\nFound {len(surebets)} surebets (profit >= 0.5%)")
    
    if surebets:
        print("\nTop 5 surebets by profit:")
        sorted_surebets = sorted(surebets, key=lambda x: x.get('profit_percent', 0), reverse=True)
        
        for i, sb in enumerate(sorted_surebets[:5], 1):
            profit = sb.get('profit_percent', 0)
            event_name = sb.get('event_name', 'Unknown Match')
            
            print(f"\n{i}. {event_name}")
            print(f"   Profit: {profit:.2f}%")
            
            for leg in sb.get('legs', []):
                bk = leg.get('bookmaker', '?')
                odds = leg.get('odds', {})
                outcome = leg.get('outcome', '?')
                print(f"   {bk}: {outcome} @ {odds}")
    
    return len(all_events), len(surebets)


if __name__ == '__main__':
    events, surebets = asyncio.run(scan_all())
