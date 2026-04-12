"""Zenit parser - outputs JSON for Rust bridge"""
import sys, json, os, time, asyncio
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

try:
    from scanner.parsers.zenit_parser import ZenitParser

    async def main():
        parser = ZenitParser()
        events = await parser.get_events()
        print(json.dumps(events, ensure_ascii=False, default=str))

    asyncio.run(main())
except Exception as e:
    print(json.dumps([]))