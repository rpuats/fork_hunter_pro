"""
Find Winline events API - try different patterns
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {
    "Accept": "application/json",
    "Referer": "https://winline.ru",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
}

# Winline uses a different API structure - try common patterns
patterns = [
    "https://winline.ru/api/events/live/football",
    "https://winline.ru/api/events/line/football",
    "https://winline.ru/api/v2/events/live/football",
    "https://winline.ru/api/v2/events/line/football",
    "https://winline.ru/api/betline/live/football",
    "https://winline.ru/api/betline/line/football",
    # Try the xds API with different event IDs
    "https://winline.ru/api/xds/v2/events?sport=1",
    "https://winline.ru/api/xds/v2/line?sport=1",
    # Try common Russian BK API patterns
    "https://winline.ru/api/v1/live/events",
    "https://winline.ru/api/v1/line/events",
    "https://api.winline.ru/v1/events/live",
    "https://api.winline.ru/v1/events/line",
]

for url in patterns:
    try:
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            ct = r.headers.get('content-type', '')
            if 'json' in ct:
                try:
                    data = r.json()
                    if isinstance(data, dict):
                        print(f"[JSON] {url[:60]} -> keys: {list(data.keys())[:8]}")
                    elif isinstance(data, list):
                        print(f"[JSON] {url[:60]} -> list[{len(data)}]")
                except:
                    print(f"[TXT] {url[:60]} -> not JSON, len={len(r.text)}")
            else:
                print(f"[HTML] {url[:60]} -> {ct}, len={len(r.text)}")
        else:
            print(f"[{r.status_code}] {url[:60]}")
    except Exception as e:
        print(f"[ERR] {url[:60]}: {type(e).__name__}")
