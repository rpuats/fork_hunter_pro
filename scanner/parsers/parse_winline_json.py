"""Winline parser - outputs JSON for Rust bridge"""
import sys, json, os, time
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

try:
    from winline_playwright import WinlinePlaywrightParser
    import asyncio

    async def main():
        async with WinlinePlaywrightParser() as parser:
            parser.urls = [
                "https://winline.ru/football",
                "https://winline.ru/live/football",
                "https://winline.ru/basketball",
                "https://winline.ru/live/basketball",
            ]
            events = await parser.get_events()
            print(json.dumps(events, ensure_ascii=False, default=str))

    asyncio.run(main())
except Exception as e:
    print(json.dumps([]))
