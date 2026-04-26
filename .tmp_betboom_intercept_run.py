import asyncio, json, sys, os
sys.path.insert(0, os.getcwd())
from scanner.parsers.betboom_intercept import BetboomInterceptParser

async def main():
    parser = BetboomInterceptParser()
    events = await parser.get_events()
    sys.stdout.buffer.write(json.dumps({'events': events[:20], 'count': len(events), 'api_calls': len(parser.api_data), 'api_urls': [x['url'] for x in parser.api_data[:40]]}, ensure_ascii=False, indent=2).encode('utf-8'))

asyncio.run(main())
