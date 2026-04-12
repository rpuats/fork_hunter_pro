"""
Test all parsers and count events
"""
import asyncio
import sys
sys.stdout.reconfigure(encoding='utf-8')

async def test_all():
    results = {}
    
    # Leon
    try:
        from scanner.parsers.leon_parser import LeonParser
        p = LeonParser()
        events = await p.get_events()
        results['Leon'] = len(events)
        print(f"Leon: {len(events)} events ✅")
    except Exception as e:
        print(f"Leon: ERROR - {e}")
    
    # OlimpBet
    try:
        from scanner.parsers.olimp_parser import OlimpParser
        p = OlimpParser()
        events = await p.get_events()
        results['OlimpBet'] = len(events)
        print(f"OlimpBet: {len(events)} events")
    except Exception as e:
        print(f"OlimpBet: ERROR - {e}")
    
    # Winline
    try:
        from scanner.parsers.winline_playwright import WinlinePlaywrightParser
        p = WinlinePlaywrightParser()
        events = await p.get_events()
        results['Winline'] = len(events)
        print(f"Winline: {len(events)} events")
    except Exception as e:
        print(f"Winline: ERROR - {e}")
    
    # Pari
    try:
        from scanner.parsers.pari_playwright import PariPlaywrightParser
        p = PariPlaywrightParser()
        events = await p.get_events()
        results['Pari'] = len(events)
        print(f"Pari: {len(events)} events")
    except Exception as e:
        print(f"Pari: ERROR - {e}")
    
    # Zenit
    try:
        from scanner.parsers.zenit_playwright import ZenitPlaywrightParser
        p = ZenitPlaywrightParser()
        events = await p.get_events()
        results['Zenit'] = len(events)
        print(f"Zenit: {len(events)} events")
    except Exception as e:
        print(f"Zenit: ERROR - {e}")
    
    # Marathon
    try:
        from scanner.parsers.marathon_playwright import MarathonPlaywrightParser
        p = MarathonPlaywrightParser()
        events = await p.get_events()
        results['Marathon'] = len(events)
        print(f"Marathon: {len(events)} events")
    except Exception as e:
        print(f"Marathon: ERROR - {e}")
    
    # Betcity
    try:
        from scanner.parsers.betcity_playwright import BetcityPlaywrightParser
        p = BetcityPlaywrightParser()
        events = await p.get_events()
        results['Betcity'] = len(events)
        print(f"Betcity: {len(events)} events")
    except Exception as e:
        print(f"Betcity: ERROR - {e}")
    
    # Baltbet
    try:
        from scanner.parsers.baltbet_playwright import BaltbetRegexParser
        p = BaltbetRegexParser()
        events = await p.get_events()
        results['Baltbet'] = len(events)
        print(f"Baltbet: {len(events)} events")
    except Exception as e:
        print(f"Baltbet: ERROR - {e}")
    
    # Bettery
    try:
        from scanner.parsers.bettery_playwright import BetteryPlaywrightParser
        p = BetteryPlaywrightParser()
        events = await p.get_events()
        results['Bettery'] = len(events)
        print(f"Bettery: {len(events)} events")
    except Exception as e:
        print(f"Bettery: ERROR - {e}")
    
    # BetBoom
    try:
        from scanner.parsers.betboom_playwright import BetBoomPlaywrightParser
        p = BetBoomPlaywrightParser()
        events = await p.get_events()
        results['BetBoom'] = len(events)
        print(f"BetBoom: {len(events)} events")
    except Exception as e:
        print(f"BetBoom: ERROR - {e}")
    
    # Fonbet
    try:
        from scanner.parsers.fonbet_playwright import FonbetPlaywrightParser
        p = FonbetPlaywrightParser()
        events = await p.get_events()
        results['Fonbet'] = len(events)
        print(f"Fonbet: {len(events)} events")
    except Exception as e:
        print(f"Fonbet: ERROR - {e}")
    
    total = sum(results.values())
    print(f"\n{'='*50}")
    print(f"TOTAL: {total} events from {len(results)} bookmakers")
    print(f"{'='*50}")

asyncio.run(test_all())
