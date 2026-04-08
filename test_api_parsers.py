"""
Quick test - only API-based parsers (no Playwright)
"""
import asyncio
import sys
sys.stdout.reconfigure(encoding='utf-8')

async def test_api_parsers():
    # Leon
    try:
        from scanner.parsers.leon_parser import LeonParser
        p = LeonParser()
        events = await p.get_events()
        print(f"Leon: {len(events)} events")
    except Exception as e:
        print(f"Leon: ERROR - {e}")
    
    # OlimpBet
    try:
        from scanner.parsers.olimp_parser import OlimpParser
        p = OlimpParser()
        events = await p.get_events()
        print(f"OlimpBet: {len(events)} events")
    except Exception as e:
        print(f"OlimpBet: ERROR - {e}")
    
    # LigaStavok (needs Playwright non-headless - skip for now)
    print("LigaStavok: needs manual captcha - skip")

asyncio.run(test_api_parsers())
