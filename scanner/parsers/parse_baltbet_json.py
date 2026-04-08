"""Baltbet parser - outputs JSON for Rust bridge"""
import sys, json, os, time, asyncio
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

try:
    from scanner.parsers.baltbet_playwright import BaltbetPlaywrightParser

    async def main():
        async with BaltbetPlaywrightParser() as parser:
            parser.urls = [
                "https://baltbet.ru/line",
                "https://baltbet.ru/live",
            ]
            events = await parser.get_events()
            print(json.dumps(events, ensure_ascii=False, default=str))

    asyncio.run(main())
except Exception as e:
    print(json.dumps([]))
