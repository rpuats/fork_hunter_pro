"""
Standalone Winline parser - outputs JSON to stdout.
Usage: python parse_winline.py <url>
"""
import asyncio
import json
import sys
import os

# Setup paths
script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
if project_root not in sys.path:
    sys.path.insert(0, project_root)

async def main():
    if len(sys.argv) < 2:
        print(json.dumps([]))
        sys.exit(0)
    
    url = sys.argv[1]
    sys.stderr.write(f"[winline] Starting for {url}\n")
    sys.stderr.flush()
    
    try:
        from scanner.parsers.winline_playwright import WinlinePlaywrightParser
        async with WinlinePlaywrightParser() as parser:
            parser.urls = [url]
            events = await parser.get_events()
        
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
        
        sys.stderr.write(f"[winline] Found {len(result)} events\n")
        sys.stderr.flush()
        print(json.dumps(result, ensure_ascii=False))
        sys.stdout.flush()
    except Exception as ex:
        import traceback
        sys.stderr.write(f"[winline] Error: {ex}\n")
        sys.stderr.write(traceback.format_exc())
        sys.stderr.flush()
        print(json.dumps([]))
        sys.stdout.flush()

if __name__ == '__main__':
    asyncio.run(main())
