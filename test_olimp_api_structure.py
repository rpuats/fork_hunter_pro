"""
OlimpBet API structure explorer
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

urls = [
    "https://www.olimp.bet/api/v4/0/live/sports-with-competitions-with-events?vids%5B%5D=",
    "https://www.olimp.bet/api/v4/0/line/sports-with-competitions-with-events?vids%5B%5D=",
]

headers = {
    "Accept": "application/json",
    "Referer": "https://www.olimp.bet",
    "Origin": "https://www.olimp.bet",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
}

for url in urls:
    try:
        r = requests.get(url, headers=headers, timeout=15)
        print(f"[{r.status_code}] {url[:80]}")
        if r.status_code == 200:
            data = r.json()
            if isinstance(data, list):
                print(f"  List with {len(data)} items")
                if len(data) > 0:
                    item = data[0]
                    print(f"  First item keys: {list(item.keys())[:10]}")
                    # Check if it has events
                    if 'events' in item:
                        events = item.get('events', [])
                        print(f"  Events in first sport: {len(events)}")
                        if len(events) > 0:
                            e = events[0]
                            print(f"  Event keys: {list(e.keys())[:15]}")
                            # Check for odds
                            if 'markets' in e:
                                markets = e.get('markets', [])
                                print(f"  Markets: {len(markets)}")
                                if len(markets) > 0:
                                    m = markets[0]
                                    print(f"  Market keys: {list(m.keys())[:10]}")
                                    if 'outcomes' in m:
                                        outcomes = m.get('outcomes', [])
                                        print(f"  Outcomes: {len(outcomes)}")
                                        for o in outcomes[:3]:
                                            print(f"    Outcome: {o}")
            elif isinstance(data, dict):
                print(f"  Dict keys: {list(data.keys())[:10]}")
    except Exception as ex:
        print(f"[ERR] {url[:80]}: {ex}")
