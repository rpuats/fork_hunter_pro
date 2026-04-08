"""
Test all API-based parsers (no Playwright)
"""
import asyncio
import sys
sys.stdout.reconfigure(encoding='utf-8')

async def test_all_api():
    results = {}
    
    # Leon
    try:
        from scanner.parsers.leon_parser import LeonParser
        p = LeonParser()
        events = await p.get_events()
        results['Leon'] = len(events)
        print(f"✅ Leon: {len(events)} events")
    except Exception as e:
        print(f"❌ Leon: ERROR - {e}")
    
    # Pari API
    try:
        from scanner.parsers.pari_api import PariParser as PariApiParser
        p = PariApiParser()
        events = await p.get_events()
        results['Pari'] = len(events)
        print(f"✅ Pari: {len(events)} events")
    except Exception as e:
        print(f"❌ Pari: ERROR - {e}")
    
    # OlimpBet API
    try:
        from scanner.parsers.olimp_parser import OlimpParser as OlimpApiParser
        p = OlimpApiParser()
        events = await p.get_events()
        results['OlimpBet'] = len(events)
        print(f"✅ OlimpBet: {len(events)} events")
    except Exception as e:
        print(f"❌ OlimpBet: ERROR - {e}")
    
    # Fonbet API
    try:
        from scanner.parsers.fonbet_api import FonbetParser as FonbetApiParser
        p = FonbetApiParser()
        events = await p.get_events()
        results['Fonbet'] = len(events)
        print(f"✅ Fonbet: {len(events)} events")
    except Exception as e:
        print(f"❌ Fonbet: ERROR - {e}")
    
    # Bettery API
    try:
        from scanner.parsers.bettery_api import BetteryParser
        p = BetteryParser()
        events = await p.get_events()
        results['Bettery'] = len(events)
        print(f"✅ Bettery: {len(events)} events")
    except Exception as e:
        print(f"❌ Bettery: ERROR - {e}")
    
    total = sum(results.values())
    print(f"\n{'='*50}")
    print(f"TOTAL: {total} events from {len(results)} bookmakers")
    print(f"{'='*50}")
    for name, count in sorted(results.items(), key=lambda x: -x[1]):
        print(f"  {name}: {count}")

asyncio.run(test_all_api())
