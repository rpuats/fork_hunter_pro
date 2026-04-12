#!/usr/bin/env python3
"""
API Parameter Tester for Legacy Bookmakers
Tests different parameter combinations for Betcity, Winline, Zenit, and Baltbet APIs
to find working combinations for parsing.
"""

import asyncio
import aiohttp
import json
import logging
from typing import List, Dict, Any, Optional
from urllib.parse import urljoin, urlencode
import time
from datetime import datetime

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)

class APITester:
    def __init__(self):
        self.session = None
        self.results = {
            'betcity': [],
            'winline': [],
            'zenit': [],
            'baltbet': []
        }

    async def init_session(self):
        """Initialize aiohttp session with headers"""
        headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            'Accept': 'application/json, text/plain, */*',
            'Accept-Language': 'ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7',
            'Cache-Control': 'no-cache',
            'X-Requested-With': 'XMLHttpRequest',
        }
        self.session = aiohttp.ClientSession(headers=headers)

    async def close_session(self):
        if self.session:
            await self.session.close()

    async def test_endpoint(self, url: str, params: Optional[Dict[str, Any]] = None, bookmaker: str = "") -> Dict[str, Any]:
        """Test a single endpoint with optional parameters"""
        full_url = url
        try:
            if params:
                full_url = f"{url}?{urlencode(params)}"
            else:
                full_url = url

            logger.debug(f"Testing {bookmaker}: {full_url}")

            async with self.session.get(full_url, timeout=aiohttp.ClientTimeout(total=10)) as resp:
                result = {
                    'url': full_url,
                    'status': resp.status,
                    'headers': dict(resp.headers),
                    'content_length': 0,
                    'params': params or {},
                    'timestamp': datetime.now().isoformat()
                }

                if resp.status == 200:
                    text_content = await resp.text()
                    result['content_length'] = len(text_content)

                    content_type = resp.headers.get('content-type', '').lower()
                    if 'json' in content_type:
                        try:
                            data = await resp.json()
                            result['data_type'] = 'json'
                            result['data_size'] = len(str(data))
                            result['has_events'] = self._check_for_events(data)
                            result['sample_keys'] = list(data.keys())[:10] if isinstance(data, dict) else []
                            result['data_preview'] = str(data)[:500] if data else ""
                        except Exception as e:
                            result['data_type'] = 'text'
                            result['parse_error'] = str(e)
                            result['text_preview'] = text_content[:500]
                    else:
                        result['data_type'] = 'html/text'
                        result['text_preview'] = text_content[:500]
                else:
                    result['error'] = f"HTTP {resp.status}"

                return result

        except aiohttp.ClientError as e:
            logger.warning(f"Client error testing {full_url}: {e}")
            return {
                'url': full_url,
                'error': f"ClientError: {str(e)}",
                'params': params or {},
                'timestamp': datetime.now().isoformat()
            }
        except asyncio.TimeoutError:
            logger.warning(f"Timeout testing {full_url}")
            return {
                'url': full_url,
                'error': "Timeout",
                'params': params or {},
                'timestamp': datetime.now().isoformat()
            }
        except Exception as e:
            logger.warning(f"Unexpected error testing {full_url}: {e}")
            return {
                'url': full_url,
                'error': f"Unexpected: {str(e)}",
                'params': params or {},
                'timestamp': datetime.now().isoformat()
            }

    def _check_for_events(self, data: Any) -> bool:
        """Check if response contains event-like data"""
        if isinstance(data, dict):
            # Look for common event indicators
            indicators = ['events', 'matches', 'tournaments', 'sports', 'line', 'live']
            return any(indicator in key.lower() for key in data.keys())
        elif isinstance(data, list):
            return len(data) > 0
        return False

    async def test_betcity(self):
        """Test Betcity API parameters"""
        logger.info("Testing Betcity APIs...")

        base_urls = [
            "https://betcity.ru/api/line",
            "https://betcity.ru/api/v1/line",
            "https://betcity.ru/api/v2/line",
            "https://betcity.ru/api/events",
            "https://betcity.ru/api/live",
            "https://betcity.ru/live-events/v2/live",
            "https://betcity.ru/line-events/v2/line"
        ]

        # Test rev values (1-5, reduce load)
        rev_values = list(range(1, 6))

        # Test add parameters
        add_values = ['1', '2', '3']

        # Test tp values for popular events
        tp_values = ['1', '2', '3', '5', '10']

        tasks = []

        for base_url in base_urls:
            # Test base URL first
            tasks.append(self.test_endpoint(base_url, None, 'betcity'))

            # Test rev parameter
            for rev in rev_values:
                tasks.append(self.test_endpoint(base_url, {'rev': rev}, 'betcity'))

            # Test add parameter
            for add in add_values:
                tasks.append(self.test_endpoint(base_url, {'add': add}, 'betcity'))

            # Test tp parameter
            for tp in tp_values:
                tasks.append(self.test_endpoint(base_url, {'tp': tp}, 'betcity'))

            # Test simple combinations
            tasks.append(self.test_endpoint(base_url, {'rev': '1', 'tp': '1'}, 'betcity'))

        results = await asyncio.gather(*tasks, return_exceptions=True)
        for result in results:
            if not isinstance(result, Exception):
                self.results['betcity'].append(result)

    async def test_winline(self):
        """Test Winline API parameters"""
        logger.info("Testing Winline APIs...")

        base_urls = [
            "https://winline.ru/api/live/events",
            "https://winline.ru/api/v2/live",
            "https://winline.ru/api/line/events",
            "https://winline.ru/api/v2/line",
            "https://winline.ru/api/tournaments",
            "https://winline.ru/api/markets"
        ]

        # Test tournament IDs (sample range)
        tournament_ids = [70000, 75000, 80000, 85000, 90000]

        # Test market types
        market_types = ['1', '2', '3', '4', '5']

        tasks = []

        for base_url in base_urls:
            # Test base URL first
            tasks.append(self.test_endpoint(base_url, None, 'winline'))

            # Test tournament IDs
            for tid in tournament_ids:
                tasks.append(self.test_endpoint(base_url, {'tournament': tid}, 'winline'))

            # Test market types
            for mt in market_types:
                tasks.append(self.test_endpoint(base_url, {'market': mt}, 'winline'))

        results = await asyncio.gather(*tasks, return_exceptions=True)

        # Extract event IDs from successful responses for further testing
        for result in results:
            if not isinstance(result, Exception) and result.get('status') == 200 and result.get('has_events'):
                try:
                    if result.get('data_type') == 'json' and 'sample_keys' in result:
                        # Could parse for event IDs here if needed
                        pass
                except:
                    pass

            if not isinstance(result, Exception):
                self.results['winline'].append(result)

    async def test_zenit(self):
        """Test Zenit API parameters"""
        logger.info("Testing Zenit APIs...")

        base_urls = [
            "https://zenit.win/api/line",
            "https://zenit.win/api/v1/line",
            "https://zenit.win/api/v2/line",
            "https://zenit.win/api/events",
            "https://zenit.win/api/live",
            "https://api.zenit.win/v1/line",
            "https://api.zenit.win/v1/live"
        ]

        # Test sport IDs
        sport_ids = ['1', '2', '3', '4', '5', '10']

        # Test imprinthash values (common patterns)
        imprinthash_values = ['abc123', 'def456', '1234567890abcdef', 'test', 'hash123']

        # Test live vs prematch
        live_prematch = ['live', 'prematch', '1', '2']

        tasks = []

        for base_url in base_urls:
            # Test base URL first
            tasks.append(self.test_endpoint(base_url, None, 'zenit'))

            # Test sport IDs
            for sid in sport_ids:
                tasks.append(self.test_endpoint(base_url, {'sport': sid}, 'zenit'))

            # Test imprinthash
            for ihash in imprinthash_values:
                tasks.append(self.test_endpoint(base_url, {'imprinthash': ihash}, 'zenit'))

            # Test live/prematch
            for lp in live_prematch:
                tasks.append(self.test_endpoint(base_url, {'type': lp}, 'zenit'))

        results = await asyncio.gather(*tasks, return_exceptions=True)
        for result in results:
            if not isinstance(result, Exception):
                self.results['zenit'].append(result)

    async def test_baltbet(self):
        """Test Baltbet API parameters"""
        logger.info("Testing Baltbet APIs...")

        base_urls = [
            "https://baltbet.ru/api/line",
            "https://baltbet.ru/api/v1/line",
            "https://baltbet.ru/api/v2/line",
            "https://baltbet.ru/api/events",
            "https://baltbet.ru/api/live",
            "https://old.baltbet.ru/api/line",
            "https://old.baltbet.ru/api/events"
        ]

        # Test pagination parameters
        page_values = ['1', '2', '3', '0']
        limit_values = ['10', '20', '50', '100']

        # Test sport filters
        sport_filters = ['1', '2', '3', '4', '5', 'football', 'basketball', 'hockey']

        tasks = []

        for base_url in base_urls:
            # Test base URL first
            tasks.append(self.test_endpoint(base_url, None, 'baltbet'))

            # Test pagination
            for page in page_values:
                tasks.append(self.test_endpoint(base_url, {'page': page}, 'baltbet'))

            for limit in limit_values:
                tasks.append(self.test_endpoint(base_url, {'limit': limit}, 'baltbet'))

            # Test sport filters
            for sport in sport_filters:
                tasks.append(self.test_endpoint(base_url, {'sport': sport}, 'baltbet'))

            # Test simple combinations
            tasks.append(self.test_endpoint(base_url, {'page': '1', 'limit': '10'}, 'baltbet'))

        results = await asyncio.gather(*tasks, return_exceptions=True)
        for result in results:
            if not isinstance(result, Exception):
                self.results['baltbet'].append(result)

    def analyze_results(self):
        """Analyze test results and print summary"""
        logger.info("Analyzing results...")

        for bookmaker, results in self.results.items():
            logger.info(f"\n=== {bookmaker.upper()} RESULTS ===")

            successful = [r for r in results if r.get('status') == 200]
            with_events = [r for r in successful if r.get('has_events')]

            logger.info(f"Total requests: {len(results)}")
            logger.info(f"Successful (200): {len(successful)}")
            logger.info(f"With events: {len(with_events)}")

            # Group by URL pattern
            url_patterns = {}
            for result in successful:
                url = result['url'].split('?')[0]  # Remove query params
                if url not in url_patterns:
                    url_patterns[url] = []
                url_patterns[url].append(result)

            logger.info(f"Successful endpoints: {len(url_patterns)}")

            # Show top working endpoints
            for url, res_list in sorted(url_patterns.items(), key=lambda x: len(x[1]), reverse=True)[:5]:
                logger.info(f"  {url}: {len(res_list)} successful requests")

            # Show working parameters for top endpoints
            if with_events:
                logger.info("Endpoints with event data:")
                for result in with_events[:10]:  # Show first 10
                    params_str = ', '.join(f"{k}={v}" for k, v in result.get('params', {}).items())
                    logger.info(f"  {result['url']} (params: {params_str}) - {result.get('data_type', 'unknown')}")

    async def save_results(self, filename: str = "api_test_results.json"):
        """Save results to JSON file"""
        with open(filename, 'w', encoding='utf-8') as f:
            json.dump(self.results, f, indent=2, ensure_ascii=False)
        logger.info(f"Results saved to {filename}")

    async def run_all_tests(self):
        """Run all API tests"""
        await self.init_session()

        try:
            await asyncio.gather(
                self.test_betcity(),
                self.test_winline(),
                self.test_zenit(),
                self.test_baltbet()
            )
        finally:
            await self.close_session()

        self.analyze_results()
        await self.save_results()

async def main():
    tester = APITester()
    await tester.run_all_tests()

if __name__ == "__main__":
    asyncio.run(main())