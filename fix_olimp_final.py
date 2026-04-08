"""
Fix OlimpBet Prematch - try to get events via sport ID
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

# Try different approaches to get line events
urls = [
    # Try with specific sport ID in vids
    "https://www.olimp.bet/api/v4/0/line/sports-with-categories-with-competitions?vids%5B%5D=1",
    "https://www.olimp.bet/api/v4/0/line/sports-with-categories-with-competitions?vids%5B%5D=1&vids%5B%5D=2&vids%5B%5D=3",
    # Try event list endpoint
    "https://www.olimp.bet/api/v4/0/line/events?lang=ru",
    "https://www.olimp.bet/api/v4/0/line/events?sport=1",
    # Try with different structure
    "https://www.olimp.bet/api/v4/0/line/sport/1/events",
    "https://www.olimp.bet/api/v4/0/line/competitions?sport=1",
    # Try the popular endpoint but with all sports
    "https://www.olimp.bet/api/v4/0/line/popular/sports-with-competitions-with-events?vids%5B%5D=",
]

for url in urls:
    try:
        r = requests.get(url, headers=headers, timeout=10)
        print(f"\n[{r.status_code}] {url[:80]}")
        if r.status_code == 200:
            try:
                d = r.json()
                if isinstance(d, list):
                    print(f"  List[{len(d)}]")
                    # Check first item structure
                    if len(d) > 0:
                        item = d[0]
                        if isinstance(item, dict):
                            payload = item.get('payload', {})
                            print(f"  Payload keys: {list(payload.keys())[:8]}")
                            # Look for events
                            for k, v in payload.items():
                                if isinstance(v, list) and len(v) > 0:
                                    print(f"  {k}: list[{len(v)}]")
                                    if isinstance(v[0], dict):
                                        print(f"    First keys: {list(v[0].keys())[:10]}")
                                elif isinstance(v, dict):
                                    print(f"  {k}: dict keys={list(v.keys())[:5]}")
                elif isinstance(d, dict):
                    print(f"  Dict keys: {list(d.keys())[:8]}")
            except Exception as e:
                print(f"  Parse error: {e}")
        else:
            print(f"  Response: {r.text[:150]}")
    except Exception as e:
        print(f"  Error: {e}")
