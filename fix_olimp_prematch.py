"""
Fix OlimpBet Prematch - try different endpoints
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

# Try different line/prematch endpoints
urls = [
    # Popular with higher limits
    "https://www.olimp.bet/api/v4/0/line/popular/sports-with-events?vids%5B%5D=&sportLimit=50&eventLimit=100",
    # Try categories endpoint for line
    "https://www.olimp.bet/api/v4/0/line/sports-with-categories-with-competitions?vids%5B%5D=",
    # Try without vids param
    "https://www.olimp.bet/api/v4/0/line/sports-with-competitions-with-events",
    # Try different API versions
    "https://www.olimp.bet/api/v4/0/line/events?sport=1",
    "https://www.olimp.bet/api/v4/0/line/sports?sport=1",
    # Try with different params
    "https://www.olimp.bet/api/v4/0/line/sports-with-competitions-with-events?lang=ru",
    "https://www.olimp.bet/api/v4/0/line/sports-with-competitions-with-events?sport=1",
]

for url in urls:
    try:
        r = requests.get(url, headers=headers, timeout=15)
        print(f"\n[{r.status_code}] {url[:80]}")
        if r.status_code == 200:
            try:
                data = r.json()
                if isinstance(data, list):
                    print(f"  List[{len(data)}]")
                    if len(data) > 0:
                        item = data[0]
                        if isinstance(item, dict):
                            payload = item.get('payload', {})
                            if isinstance(payload, dict):
                                print(f"  Payload keys: {list(payload.keys())[:8]}")
                                # Count events
                                total_events = 0
                                comps = payload.get('competitionsWithEvents', [])
                                if isinstance(comps, list):
                                    for c in comps:
                                        evts = c.get('events', [])
                                        if isinstance(evts, list):
                                            total_events += len(evts)
                                print(f"  Total events: {total_events}")
                elif isinstance(data, dict):
                    print(f"  Dict keys: {list(data.keys())[:8]}")
            except Exception as e:
                print(f"  Parse error: {e}")
        else:
            print(f"  Response: {r.text[:200]}")
    except Exception as e:
        print(f"  Error: {e}")
