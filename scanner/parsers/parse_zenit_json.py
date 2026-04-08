"""Zenit parser - outputs JSON for Rust bridge"""
import sys, json, os, time, asyncio
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

try:
    from scanner.parsers.zenit_playwright import ZenitPlaywrightParser

    async def main():
        async with ZenitPlaywrightParser() as parser:
            parser.urls = [
                "https://zenit.win/line/football",
                "https://zenit.win/live/football",
            ]
            events = await parser.get_events()
            print(json.dumps(events, ensure_ascii=False, default=str))

    asyncio.run(main())
except Exception as e:
    print(json.dumps([]))
