import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://winline.ru", "User-Agent": "Mozilla/5.0"}

# Try to find the main events API
# Winline might use WebSocket or a different structure
# Let's try common patterns

patterns = [
    # Try the alter endpoint more carefully
    "https://winline.ru/api/static-data/alter/1/80632",
    # Try different API versions
    "https://winline.ru/api/v2/line",
    "https://winline.ru/api/v2/live",
    "https://winline.ru/api/line",
    "https://winline.ru/api/live",
    # Try xds with different params
    "https://winline.ru/api/xds/v2/line?sport=5",
    "https://winline.ru/api/xds/v2/live?sport=5",
    # Try cls with different patterns
    "https://winline.ru/api/cls/line?sport=5",
    "https://winline.ru/api/cls/live?sport=5",
    # Try events endpoint
    "https://winline.ru/api/events",
    "https://winline.ru/api/events?sport=5",
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
                print(f"[{ct.split(';')[0]}] {url[:60]} -> len={len(r.text)}")
        else:
            print(f"[{r.status_code}] {url[:60]}")
    except Exception as e:
        print(f"[ERR] {url[:60]}: {type(e).__name__}")
