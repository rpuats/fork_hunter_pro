#!/usr/bin/env python3
"""
Test Winline API endpoints directly
"""
import requests
import json
import sys
import time
from typing import Dict, Any

sys.stdout.reconfigure(encoding='utf-8')

class WinlineAPITester:
    def __init__(self):
        self.headers = {
            'Accept': 'application/json',
            'Referer': 'https://winline.ru',
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            'Accept-Language': 'ru-RU,ru;q=0.9,en;q=0.8'
        }

    def test_endpoint(self, url: str) -> Dict[str, Any]:
        """Test a single endpoint"""
        result = {
            'url': url,
            'status': None,
            'content_type': None,
            'size': 0,
            'is_json': False,
            'json_structure': None,
            'has_events': False,
            'event_count': 0,
            'error': None
        }

        try:
            response = requests.get(url, headers=self.headers, timeout=15)
            result['status'] = response.status_code
            result['content_type'] = response.headers.get('content-type', '')

            if response.status_code == 200:
                result['size'] = len(response.text)

                # Check if it's JSON
                try:
                    data = response.json()
                    result['is_json'] = True
                    result['json_structure'] = self.analyze_json_structure(data)
                    result['has_events'], result['event_count'] = self.check_for_events(data)
                except json.JSONDecodeError:
                    result['is_json'] = False
                    if 'html' in result['content_type'].lower():
                        result['json_structure'] = 'HTML'
                    else:
                        result['json_structure'] = f'Non-JSON: {result["content_type"]}'

            else:
                result['error'] = f"HTTP {response.status_code}"

        except Exception as e:
            result['error'] = str(type(e).__name__) + ": " + str(e)

        return result

    def analyze_json_structure(self, data: Any) -> str:
        """Analyze the structure of JSON data"""
        if isinstance(data, dict):
            keys = list(data.keys())
            structure = f"dict with {len(keys)} keys: {keys[:5]}"
            if len(keys) > 5:
                structure += "..."
            return structure
        elif isinstance(data, list):
            if len(data) == 0:
                return "empty list"
            elif isinstance(data[0], dict):
                item_keys = list(data[0].keys())
                return f"list[{len(data)}] of dicts with keys: {item_keys[:5]}"
            else:
                return f"list[{len(data)}] of {type(data[0]).__name__}"
        else:
            return f"{type(data).__name__}"

    def check_for_events(self, data: Any) -> tuple[bool, int]:
        """Check if the data contains betting events"""
        if not isinstance(data, (dict, list)):
            return False, 0

        # Look for common event-related keys
        event_indicators = [
            'events', 'matches', 'games', 'sports', 'leagues',
            'home_team', 'away_team', 'odds', 'coefficients', 'k1', 'k2'
        ]

        candidates = []

        if isinstance(data, dict):
            for key in data:
                if key.lower() in ['events', 'matches', 'data', 'items', 'sports']:
                    value = data[key]
                    if isinstance(value, list):
                        candidates.extend(value)
                elif isinstance(data[key], list) and len(data[key]) > 0:
                    # Check if items look like events
                    item = data[key][0]
                    if isinstance(item, dict) and any(k in item for k in ['home', 'away', 'team1', 'team2', 'odds']):
                        candidates.extend(data[key])
        elif isinstance(data, list):
            candidates = data

        # Validate candidates
        valid_events = 0
        for item in candidates[:50]:  # Check first 50 items
            if isinstance(item, dict):
                # Check for team names and odds
                has_teams = any(k in item for k in ['home_team', 'away_team', 'home', 'away', 'team1', 'team2'])
                has_odds = any(k in item for k in ['odds', 'k1', 'k2', 'coefficient1', 'coefficient2'])

                if has_teams and has_odds:
                    valid_events += 1

        return valid_events > 0, valid_events

    def run_tests(self):
        """Run tests on various API endpoints"""
        # Known patterns from the codebase
        api_patterns = [
            # Standard API endpoints
            "https://winline.ru/api/events/live",
            "https://winline.ru/api/events/line",
            "https://winline.ru/api/v2/events/live",
            "https://winline.ru/api/v2/events/line",
            "https://winline.ru/api/betline/events",
            "https://winline.ru/api/betline/live",
            "https://winline.ru/api/betline/line",

            # Static data endpoints
            "https://winline.ru/api/static-data/alter/1/80632",

            # Try different versions
            "https://winline.ru/api/v3/events/live",
            "https://winline.ru/api/v3/events/line",

            # Try different paths
            "https://winline.ru/api/data/events",
            "https://winline.ru/api/data/live",
            "https://winline.ru/api/data/line",

            # Try sports-specific
            "https://winline.ru/api/sports/football/events",
            "https://winline.ru/api/sports/1/events",  # Football ID might be 1

            # GraphQL-like
            "https://winline.ru/graphql",

            # AJAX endpoints
            "https://winline.ru/ajax/events",
            "https://winline.ru/ajax/live",
            "https://winline.ru/ajax/line",
        ]

        print("Testing Winline API endpoints...")
        print("=" * 80)

        working_endpoints = []

        for url in api_patterns:
            print(f"\nTesting: {url}")
            result = self.test_endpoint(url)

            if result['status'] == 200:
                print(f"  ✓ Status: {result['status']}")
                print(f"  Content-Type: {result['content_type']}")
                print(f"  Size: {result['size']:,} bytes")
                print(f"  Is JSON: {result['is_json']}")

                if result['is_json']:
                    print(f"  Structure: {result['json_structure']}")
                    if result['has_events']:
                        print(f"  ✓ EVENTS FOUND: {result['event_count']} events")
                        working_endpoints.append(result)
                    else:
                        print("  ✗ No events detected")
                else:
                    print(f"  Content: {result['json_structure']}")
            else:
                print(f"  ✗ Status: {result['status']} - {result['error']}")

            time.sleep(0.5)  # Rate limiting

        print("\n" + "=" * 80)
        print(f"Summary: Found {len(working_endpoints)} endpoints with events")

        if working_endpoints:
            print("\nWorking endpoints with events:")
            for ep in working_endpoints:
                print(f"  {ep['url']} - {ep['event_count']} events")

if __name__ == "__main__":
    tester = WinlineAPITester()
    tester.run_tests()</content>
<parameter name="filePath">C:\Users\Administrator\Desktop\ai\Grok вилки\fork_hunter_pro\test_winline_apis_direct.py