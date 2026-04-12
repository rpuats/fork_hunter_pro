import requests
import json

endpoints = [
    "https://winline.ru/api/events/live",
    "https://winline.ru/api/events/line",
    "https://winline.ru/api/v2/events/live",
    "https://winline.ru/api/v2/events/line",
    "https://winline.ru/api/betline/live",
    "https://winline.ru/api/betline/line",
    "https://winline.ru/api/static-data/alter/1/80632"
]

headers = {
    'Accept': 'application/json',
    'Referer': 'https://winline.ru',
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
}

for url in endpoints:
    try:
        response = requests.get(url, headers=headers, timeout=10)
        print(f"\n{url}")
        print(f"Status: {response.status_code}")

        if response.status_code == 200:
            content_type = response.headers.get('content-type', '')
            print(f"Content-Type: {content_type}")

            if 'json' in content_type:
                try:
                    data = response.json()
                    print(f"✓ JSON parsed, size: {len(json.dumps(data))}")

                    if isinstance(data, dict):
                        keys = list(data.keys())
                        print(f"Keys: {keys}")

                        # Check for event-like data
                        event_keys = ['events', 'matches', 'data', 'items']
                        for key in event_keys:
                            if key in data and isinstance(data[key], list):
                                print(f"  {key}: {len(data[key])} items")
                                if len(data[key]) > 0 and isinstance(data[key][0], dict):
                                    item_keys = list(data[key][0].keys())
                                    print(f"    Item keys: {item_keys[:8]}")
                                    # Check if looks like betting event
                                    event_indicators = ['home', 'away', 'odds', 'k1', 'k2']
                                    if any(ind in str(item_keys).lower() for ind in event_indicators):
                                        print("    ✓ Looks like betting events!")
                    elif isinstance(data, list):
                        print(f"List with {len(data)} items")
                        if len(data) > 0 and isinstance(data[0], dict):
                            print(f"Item keys: {list(data[0].keys())[:8]}")

                except json.JSONDecodeError:
                    print("✗ Failed to parse JSON")
            else:
                print(f"Non-JSON content, first 100 chars: {response.text[:100]}")
        else:
            print(f"✗ HTTP {response.status_code}")

    except Exception as e:
        print(f"✗ Error: {e}")

    import time
    time.sleep(0.5)