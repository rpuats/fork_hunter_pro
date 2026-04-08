"""
Explore Winline API structure
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Winline API endpoints found
urls = [
    "https://winline.ru/api/static-data/alter/1/80632",
    "https://winline.ru/api/xds/v2/event/15500671/1",
]

# Try to find the main events API
# Winline uses a different structure - let's try common patterns
base = "https://winline.ru/api"

patterns = [
    f"{base}/events/live",
    f"{base}/events/line",
    f"{base}/v2/events/live",
    f"{base}/v2/events/line",
    f"{base}/betline/live",
    f"{base}/betline/line",
]

headers = {
    "Accept": "application/json",
    "Referer": "https://winline.ru",
    "User-Agent": "Mozilla/5.0",
}

for url in patterns:
    try:
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            try:
                data = r.json()
                if isinstance(data, dict):
                    print(f"[OK] {url} -> keys: {list(data.keys())[:8]}")
                elif isinstance(data, list):
                    print(f"[OK] {url} -> list[{len(data)}]")
            except:
                print(f"[OK] {url} -> not JSON, len={len(r.text)}")
        else:
            print(f"[{r.status_code}] {url}")
    except Exception as e:
        print(f"[ERR] {url}: {type(e).__name__}")

# Also try the alter endpoint which returned 76KB
print("\n\nChecking alter endpoint structure...")
r = requests.get("https://winline.ru/api/static-data/alter/1/80632", headers=headers, timeout=15)
if r.status_code == 200:
    data = r.json()
    print(f"Keys: {list(data.keys())}")
    for k, v in data.items():
        if isinstance(v, list):
            print(f"  {k}: list[{len(v)}]")
            if len(v) > 0 and isinstance(v[0], dict):
                print(f"    First keys: {list(v[0].keys())[:10]}")
        elif isinstance(v, dict):
            print(f"  {k}: dict keys={list(v.keys())[:5]}")
