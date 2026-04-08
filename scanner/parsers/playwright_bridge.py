"""
HTTP Bridge Server for Playwright parsers.
Keeps browser alive and serves parse requests via HTTP.
Usage: python playwright_bridge.py [port]
Endpoints:
  GET /health -> {"status": "ok"}
  GET /parse?parser=winline&url=... -> JSON array of events
"""
import asyncio
import json
import sys
import os
import threading
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs
from concurrent.futures import ThreadPoolExecutor

# Setup paths
script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
if project_root not in sys.path:
    sys.path.insert(0, project_root)

# Global executor
executor = ThreadPoolExecutor(max_workers=4)

def run_async(coro):
    """Run async coroutine in new event loop"""
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        return loop.run_until_complete(coro)
    finally:
        loop.close()

async def _parse_winline():
    from scanner.parsers.winline_playwright import WinlinePlaywrightParser
    async with WinlinePlaywrightParser() as parser:
        parser.urls = [
            "https://winline.ru/football",
            "https://winline.ru/live/football",
            "https://winline.ru/hockey",
            "https://winline.ru/live/hockey",
        ]
        return await parser.get_events()

async def _parse_zenit():
    from scanner.parsers.zenit_playwright import ZenitPlaywrightParser
    async with ZenitPlaywrightParser() as parser:
        parser.urls = ["https://zenit.win/football", "https://zenit.win/live/football"]
        return await parser.get_events()

async def _parse_betcity():
    from scanner.parsers.betcity_playwright import BetcityPlaywrightParser
    async with BetcityPlaywrightParser() as parser:
        parser.urls = ["https://betcity.ru/ru/line/football", "https://betcity.ru/ru/live/football"]
        return await parser.get_events()

async def _parse_baltbet():
    from scanner.parsers.baltbet_playwright import BaltbetPlaywrightParser
    async with BaltbetPlaywrightParser() as parser:
        parser.urls = ["https://baltbet.ru/line", "https://baltbet.ru/live"]
        return await parser.get_events()

def convert_events(events):
    """Convert parser events to simple format"""
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
    return result

def handle_parse(parser_name):
    parsers = {
        'winline': _parse_winline,
        'zenit': _parse_zenit,
        'betcity': _parse_betcity,
        'baltbet': _parse_baltbet,
    }
    coro_func = parsers.get(parser_name)
    if not coro_func:
        return []
    try:
        sys.stderr.write(f"[bridge] Parsing {parser_name}...\n")
        sys.stderr.flush()
        events = run_async(coro_func())
        result = convert_events(events)
        sys.stderr.write(f"[bridge] {parser_name}: {len(result)} events\n")
        sys.stderr.flush()
        return result
    except Exception as ex:
        import traceback
        sys.stderr.write(f"[bridge] {parser_name} error: {ex}\n")
        sys.stderr.write(traceback.format_exc())
        sys.stderr.flush()
        return []

class BridgeHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urlparse(self.path)
        params = parse_qs(parsed.query)
        
        if parsed.path == '/parse':
            parser_name = params.get('parser', [''])[0]
            if not parser_name:
                self.send_response(400)
                self.send_header('Content-Type', 'application/json')
                self.end_headers()
                self.wfile.write(json.dumps({'error': 'Missing parser'}).encode())
                return
            
            # Submit to thread pool and wait for result
            future = executor.submit(handle_parse, parser_name)
            events = future.result()
            
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(events, ensure_ascii=False).encode())
        elif parsed.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps({'status': 'ok'}).encode())
        else:
            self.send_response(404)
            self.end_headers()
    
    def log_message(self, format, *args):
        pass  # Suppress logging

def run_server(port=9876):
    server = HTTPServer(('127.0.0.1', port), BridgeHandler)
    server.request_queue_size = 10
    print(f"[bridge] Server running on http://127.0.0.1:{port}", file=sys.stderr)
    sys.stderr.flush()
    server.serve_forever()

if __name__ == '__main__':
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9876
    run_server(port)
