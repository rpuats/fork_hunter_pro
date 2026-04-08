import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://winline.ru", "User-Agent": "Mozilla/5.0"}

# Test Winline menu endpoints
urls = [
    "https://winline.ru/api/cls/menu/sport/5/sport-country-xy/8-24?theme=desktop",
    "https://winline.ru/api/cls/menu/sport/5/sport-country-xy/9-24?theme=desktop",
    "https://winline.ru/api/cls/menu/sport/205/sport-country-xy/0-24?theme=desktop",
    "https://winline.ru/api/cls/event/2/15503995",
    "https://winline.ru/api/cls/event/1/15490007",
    "https://winline.ru/api/xds/v2/event/15500658/1",
]

for url in urls:
    try:
        r = requests.get(url, headers=headers, timeout=10)
        print(f"\n[{r.status_code}] {url[:70]}")
        if r.status_code == 200:
            try:
                data = r.json()
                if isinstance(data, dict):
                    print(f"  Keys: {list(data.keys())[:10]}")
                    # Check for events
                    for k, v in data.items():
                        if isinstance(v, list):
                            print(f"  {k}: list[{len(v)}]")
                            if len(v) > 0 and isinstance(v[0], dict):
                                print(f"    First keys: {list(v[0].keys())[:10]}")
                        elif isinstance(v, dict):
                            print(f"  {k}: dict keys={list(v.keys())[:5]}")
                elif isinstance(data, list):
                    print(f"  List[{len(data)}]")
                    if len(data) > 0 and isinstance(data[0], dict):
                        print(f"  First keys: {list(data[0].keys())[:10]}")
            except:
                print(f"  Not JSON, len={len(r.text)}")
    except Exception as e:
        print(f"[ERR] {url[:70]}: {e}")

# Try to find the main events list - maybe it's under a different path
print("\n\n=== Trying to find main events list ===")
# Try different sport IDs and patterns
patterns = [
    "https://winline.ru/api/cls/menu/sport/5/sport-country-xy/0-24?theme=desktop",
    "https://winline.ru/api/cls/menu/sport/5/sport-country-xy/1-24?theme=desktop",
    "https://winline.ru/api/cls/menu/sport/5/sport-country-xy/0-23?theme=desktop",
]

for url in patterns:
    try:
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            data = r.json()
            if isinstance(data, dict):
                print(f"[OK] {url[:70]} -> keys: {list(data.keys())[:8]}")
            elif isinstance(data, list):
                print(f"[OK] {url[:70]} -> list[{len(data)}]")
    except:
        pass
