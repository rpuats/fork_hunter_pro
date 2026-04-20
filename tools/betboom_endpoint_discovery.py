#!/usr/bin/env python3
"""
BetBoom HTTP Endpoint Discovery Tool

Probes potential HTTP API endpoints to find working data feeds
instead of relying on WebSocket Protobuf decoder.
"""

import asyncio
import aiohttp
import json
from urllib.parse import urljoin
from typing import Optional, Dict, List
import sys

# Potential BetBoom API endpoints
ENDPOINTS = [
    # Common API paths
    '/api/events',
    '/api/v1/events',
    '/api/v2/events',
    '/api/v3/events',
    '/api/markets',
    '/api/v1/markets',
    '/api/v2/markets',
    '/api/odds',
    '/api/v1/odds',
    '/api/sports',
    '/api/v1/sports',
    '/api/competitions',
    '/api/v1/competitions',
    '/api/matches',
    '/api/v1/matches',
    
    # Specific sport paths
    '/api/football/events',
    '/api/v1/football/events',
    '/api/sports/football/events',
    '/api/basketball/events',
    '/api/tennis/events',
    
    # Live/prematch paths
    '/api/live',
    '/api/v1/live',
    '/api/prematch',
    '/api/v1/prematch',
    '/api/live/events',
    '/api/v1/live/events',
    
    # Alternative structures
    '/graphql',
    '/api/graphql',
    '/api/v1/graphql',
    '/betting/api/events',
    '/betting/api/odds',
    '/sportsbook/api/events',
]

BASE_URLS = [
    'https://betboom.ru',
    'https://www.betboom.ru',
    'https://api.betboom.ru',
    'https://betboom.com',
    'https://www.betboom.com',
]

class BetBoomProber:
    def __init__(self, timeout=10):
        self.timeout = timeout
        self.headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            'Accept': 'application/json, text/plain, */*',
            'Accept-Language': 'en-US,en;q=0.9',
            'Cache-Control': 'no-cache',
            'Pragma': 'no-cache',
        }
        self.results = {
            'found_endpoints': [],
            'successful_responses': [],
            'errors': [],
        }

    async def probe_endpoint(self, session: aiohttp.ClientSession, url: str) -> Optional[Dict]:
        """Probe a single endpoint"""
        try:
            async with session.get(url, timeout=self.timeout, headers=self.headers) as resp:
                if resp.status == 200:
                    try:
                        data = await resp.json()
                        return {
                            'url': url,
                            'status': resp.status,
                            'content_type': resp.content_type,
                            'size': len(await resp.text()),
                            'has_events': 'event' in json.dumps(data).lower() or 'match' in json.dumps(data).lower(),
                            'sample': str(data)[:200] if isinstance(data, dict) else str(data)[:200],
                        }
                    except:
                        text = await resp.text()
                        return {
                            'url': url,
                            'status': resp.status,
                            'content_type': resp.content_type,
                            'error': 'Failed to parse JSON',
                            'size': len(text),
                        }
                else:
                    return {
                        'url': url,
                        'status': resp.status,
                        'error': f'HTTP {resp.status}',
                    }
        except asyncio.TimeoutError:
            return {
                'url': url,
                'error': 'Timeout',
            }
        except Exception as e:
            return {
                'url': url,
                'error': str(e),
            }

    async def probe_all(self):
        """Probe all endpoints"""
        print("[*] Starting BetBoom HTTP endpoint discovery...")
        
        tasks = []
        async with aiohttp.ClientSession() as session:
            for base_url in BASE_URLS:
                for endpoint in ENDPOINTS:
                    url = urljoin(base_url, endpoint)
                    tasks.append(self.probe_endpoint(session, url))
            
            print(f"[*] Probing {len(tasks)} endpoints...")
            results = await asyncio.gather(*tasks)
            
            # Process results
            for result in results:
                if result:
                    if 'error' in result and result['error'] != 'HTTP 404':
                        # Log non-404 errors
                        if 'has_events' in result and result.get('has_events'):
                            self.results['successful_responses'].append(result)
                            print(f"[+] FOUND: {result['url']} (status: {result.get('status')})")
                        elif result.get('status') not in [404, 403]:
                            self.results['found_endpoints'].append(result)
                            print(f"[?] RESPONSE: {result['url']} (status: {result.get('status')})")
        
        # Print summary
        print("\n[=== SUMMARY ===]")
        print(f"Found endpoints: {len(self.results['found_endpoints'])}")
        print(f"Successful data endpoints: {len(self.results['successful_responses'])}")
        
        if self.results['successful_responses']:
            print("\n[+] WORKING ENDPOINTS:")
            for result in self.results['successful_responses']:
                print(f"  - {result['url']} (Status: {result.get('status')})")
                if 'sample' in result:
                    print(f"    Sample: {result['sample'][:100]}...")
        
        # Save results
        with open('betboom_endpoint_discovery.json', 'w', encoding='utf-8') as f:
            json.dump(self.results, f, indent=2)
        print("\n[OK] Results saved to betboom_endpoint_discovery.json")
        
        return self.results


async def main():
    if sys.platform == 'win32':
        # For Windows, use ProactorEventLoop for better performance
        asyncio.set_event_loop_policy(asyncio.WindowsProactorEventLoopPolicy())
    
    prober = BetBoomProber(timeout=10)
    await prober.probe_all()


if __name__ == '__main__':
    asyncio.run(main())
