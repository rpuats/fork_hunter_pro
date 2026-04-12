#!/usr/bin/env python3
"""
Smart API Discovery Script for Bookmakers
Discovers working API endpoints for Winline, Zenit, Betcity, and Baltbet
"""

import requests
import json
import time
import sys
from typing import Dict, List, Optional

class SmartAPIDiscovery:
    def __init__(self):
        self.session = requests.Session()
        self.session.headers.update({
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36',
            'Accept': 'application/json, text/plain, */*',
            'Accept-Language': 'ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7',
        })
        self.working_endpoints = {}

    def test_endpoint(self, url: str, method: str = 'GET', headers: Optional[Dict] = None, params: Optional[Dict] = None, data: Optional[Dict] = None, timeout: int = 10) -> Optional[Dict]:
        """Test an endpoint and return response info if successful"""
        try:
            req_headers = headers or {}
            req_headers.update(self.session.headers)

            response = self.session.request(
                method=method,
                url=url,
                headers=req_headers,
                params=params,
                json=data,
                timeout=timeout
            )

            if response.status_code == 200:
                content_type = response.headers.get('content-type', '').lower()
                is_json = 'json' in content_type
                is_html = 'html' in content_type

                result = {
                    'url': url,
                    'status': response.status_code,
                    'content_type': content_type,
                    'content_length': len(response.content),
                    'is_json': is_json,
                    'is_html': is_html
                }

                if is_json:
                    try:
                        result['data'] = response.json()
                        result['data_keys'] = list(result['data'].keys()) if isinstance(result['data'], dict) else f'list[{len(result["data"])}]'
                    except:
                        result['data'] = response.text[:500]
                elif is_html:
                    result['title'] = response.text.split('<title>')[1].split('</title>')[0] if '<title>' in response.text else 'No title'

                return result

        except Exception as e:
            pass

        return None

    def discover_winline(self) -> List[Dict]:
        """Discover Winline API endpoints by trying tournament IDs"""
        print("Discovering Winline endpoints...")
        working = []

        # Try the alter endpoint with tournament IDs from 80000 to 90000
        # To avoid too many requests, try every 100 IDs and sample some
        base_url = "https://winline.ru/api/static-data/alter/1/{}"

        # First try a few known working IDs
        known_ids = [80632, 85000, 86000, 87000, 88000, 89000]
        for tid in known_ids:
            url = base_url.format(tid)
            result = self.test_endpoint(url)
            if result:
                print(f"Found working Winline endpoint: {url}")
                working.append(result)
            time.sleep(0.5)  # Rate limiting

        # If no working, try range with larger steps
        if not working:
            for tid in range(80000, 90001, 1000):
                url = base_url.format(tid)
                result = self.test_endpoint(url)
                if result:
                    print(f"Found working Winline endpoint: {url}")
                    working.append(result)
                    # Once found one, try nearby IDs
                    for offset in range(-50, 51, 10):
                        if offset == 0:
                            continue
                        nearby_tid = tid + offset
                        if 80000 <= nearby_tid <= 90000:
                            nearby_url = base_url.format(nearby_tid)
                            nearby_result = self.test_endpoint(nearby_url)
                            if nearby_result:
                                working.append(nearby_result)
                    break
                time.sleep(0.1)

        return working

    def discover_zenit(self) -> List[Dict]:
        """Discover Zenit API endpoints"""
        print("Discovering Zenit endpoints...")
        working = []

        # First get the main page to find imprinthash
        try:
            main_page = self.session.get("https://zenit.win/line/football", timeout=15)
            if main_page.status_code == 200:
                html = main_page.text
                # Look for imprinthash in the HTML
                if 'imprinthash' in html:
                    # Extract imprinthash - it might be in a script tag or data attribute
                    import re
                    imprinthash_match = re.search(r'imprinthash["\']?\s*[:=]\s*["\']([^"\']+)["\']', html)
                    if imprinthash_match:
                        imprinthash = imprinthash_match.group(1)
                        print(f"Found Zenit imprinthash: {imprinthash}")

                        # Try the printer endpoint with imprinthash
                        url = "https://zenit.win/ajax/line/printer/react"
                        headers = {'imprinthash': imprinthash}
                        result = self.test_endpoint(url, headers=headers)
                        if result:
                            print(f"Found working Zenit endpoint: {url}")
                            working.append(result)
        except Exception as e:
            print(f"Error discovering Zenit: {e}")

        # Try other common API patterns
        common_urls = [
            "https://zenit.win/api/line",
            "https://zenit.win/api/v1/line",
            "https://zenit.win/api/events",
            "https://api.zenit.win/v1/line"
        ]

        for url in common_urls:
            result = self.test_endpoint(url)
            if result:
                print(f"Found working Zenit endpoint: {url}")
                working.append(result)
            time.sleep(0.5)

        return working

    def discover_betcity(self) -> List[Dict]:
        """Discover Betcity API endpoints"""
        print("Discovering Betcity endpoints...")
        working = []

        # Try various API patterns
        api_patterns = [
            "https://betcity.ru/api/line",
            "https://betcity.ru/api/events",
            "https://betcity.ru/api/v1/line",
            "https://betcity.ru/api/v2/line",
            "https://betcity.ru/events/list",
            "https://betcity.ru/events/listBase",
            "https://betcity.ru/federation/v1/get-contents",
            "https://betcity.ru/live-events/v2/live",
            "https://betcity.ru/line-events/v2/line",
            "https://betcity.ru/api/odds",
            "https://betcity.ru/api/live"
        ]

        for url in api_patterns:
            result = self.test_endpoint(url)
            if result:
                print(f"Found working Betcity endpoint: {url}")
                working.append(result)
            time.sleep(0.5)

        # Try HTML parsing approach
        try:
            html_result = self.test_endpoint("https://betcity.ru/ru/line/football")
            if html_result and html_result.get('is_html'):
                print("Betcity HTML parsing possible")
                working.append(html_result)
        except:
            pass

        return working

    def discover_baltbet(self) -> List[Dict]:
        """Discover Baltbet API endpoints"""
        print("Discovering Baltbet endpoints...")
        working = []

        # Check old site for API endpoints
        old_site_urls = [
            "https://old.baltbet.ru/api/line",
            "https://old.baltbet.ru/api/events",
            "https://old.baltbet.ru/events/list",
            "https://old.baltbet.ru/events/listBase"
        ]

        for url in old_site_urls:
            result = self.test_endpoint(url)
            if result:
                print(f"Found working Baltbet (old) endpoint: {url}")
                working.append(result)
            time.sleep(0.5)

        # Try new site
        new_site_urls = [
            "https://baltbet.ru/api/line",
            "https://baltbet.ru/api/events",
            "https://baltbet.ru/api/v1/line",
            "https://baltbet.ru/line"
        ]

        for url in new_site_urls:
            result = self.test_endpoint(url)
            if result:
                print(f"Found working Baltbet (new) endpoint: {url}")
                working.append(result)
            time.sleep(0.5)

        # Try HTML parsing
        try:
            html_result = self.test_endpoint("https://old.baltbet.ru/")
            if html_result and html_result.get('is_html'):
                print("Baltbet old site HTML parsing possible")
                working.append(html_result)
        except:
            pass

        return working

    def run_discovery(self):
        """Run discovery for all bookmakers"""
        bookmakers = {
            'winline': self.discover_winline,
            'zenit': self.discover_zenit,
            'betcity': self.discover_betcity,
            'baltbet': self.discover_baltbet
        }

        results = {}
        for bk, discover_func in bookmakers.items():
            print(f"\nStarting discovery for {bk.upper()}")
            try:
                endpoints = discover_func()
                results[bk] = endpoints
                print(f"Found {len(endpoints)} working endpoints for {bk}")
            except Exception as e:
                print(f"Error discovering {bk}: {e}")
                results[bk] = []

        # Save results
        with open('api_discovery_results.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, indent=2, ensure_ascii=False)

        print("\nResults saved to api_discovery_results.json")

        # Print summary
        print("\nSUMMARY:")
        for bk, endpoints in results.items():
            print(f"{bk.upper()}: {len(endpoints)} endpoints")
            for ep in endpoints[:3]:  # Show first 3
                print(f"  - {ep['url']}")

        return results

def main():
    print("Smart API Discovery for Bookmakers")
    print("=" * 50)

    discovery = SmartAPIDiscovery()
    results = discovery.run_discovery()

    print("\nDiscovery complete!")

if __name__ == "__main__":
    main()