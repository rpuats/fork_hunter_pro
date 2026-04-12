"""
Simple bridge: call Playwright parser and output JSON only.
Usage: python json_bridge.py <parser> <url>
Output: JSON array to stdout, errors to stderr
"""
import asyncio
import json
import sys
import os
import traceback

# Setup paths
script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
if project_root not in sys.path:
    sys.path.insert(0, project_root)

def log_error(msg):
    sys.stderr.write(f"BRIDGE_ERROR: {msg}\n")
    sys.stderr.flush()

async def parse_winline(url):
    from scanner.parsers.winline_playwright import WinlinePlaywrightParser
    async with WinlinePlaywrightParser() as parser:
        parser.urls = [url]
        events = await parser.get_events()
    return events

async def parse_zenit(url):
    from scanner.parsers.zenit_playwright import ZenitPlaywrightParser
    async with ZenitPlaywrightParser() as parser:
        parser.urls = [url]
        events = await parser.get_events()
    return events

async def parse_betcity(url):
    from scanner.parsers.betcity_playwright import BetcityPlaywrightParser
    async with BetcityPlaywrightParser() as parser:
        parser.urls = [url]
        events = await parser.get_events()
    return events

async def parse_baltbet(url):
    from scanner.parsers.baltbet_playwright import BaltbetRegexParser
    async with BaltbetRegexParser() as parser:
        parser.urls = [url]
        events = await parser.get_events()
    return events

async def main():
    if len(sys.argv) < 3:
        print(json.dumps([]))
        sys.exit(0)
    
    parser_name = sys.argv[1]
    url = sys.argv[2]
    
    sys.stderr.write(f"BRIDGE_START: {parser_name} {url}\n")
    sys.stderr.flush()
    
    try:
        if parser_name == 'winline':
            events = await parse_winline(url)
        elif parser_name == 'zenit':
            events = await parse_zenit(url)
        elif parser_name == 'betcity':
            events = await parse_betcity(url)
        elif parser_name == 'baltbet':
            events = await parse_baltbet(url)
        else:
            events = []
        
        sys.stderr.write(f"BRIDGE_GOT: {len(events)} events\n")
        sys.stderr.flush()
        
        # Convert to simple format for Rust
        result = []
        for e in events:
            result.append({
                'home_team': e.get('home_team', ''),
                'away_team': e.get('away_team', ''),
                'league': e.get('league', ''),
                'home_odds': e.get('home_odds'),
                'draw_odds': e.get('draw_odds'),
                'away_odds': e.get('away_odds'),
                'is_live': e.get('is_live', False),
            })
        
        # Output JSON ONLY to stdout
        sys.stderr.write(f"BRIDGE_OUTPUT: {len(result)} events\n")
        sys.stderr.flush()
        print(json.dumps(result, ensure_ascii=False))
        sys.stdout.flush()
        
    except Exception as ex:
        log_error(f"{parser_name}: {ex}")
        log_error(traceback.format_exc())
        print(json.dumps([]))
        sys.stdout.flush()

if __name__ == '__main__':
    asyncio.run(main())
