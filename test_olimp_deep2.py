import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://www.olimp.bet/api/v4/0/live/sports-with-competitions-with-events?vids%5B%5D="
headers = {
    "Accept": "application/json",
    "Referer": "https://www.olimp.bet",
    "User-Agent": "Mozilla/5.0",
}

r = requests.get(url, headers=headers, timeout=15)
data = r.json()

print(f"Response type: {type(data).__name__}")
print(f"Items: {len(data)}")

for i, item in enumerate(data[:3]):
    print(f"\n--- Item {i} ---")
    print(f"  operationId: {item.get('operationId')}")
    print(f"  version: {item.get('version')}")
    payload = item.get('payload')
    print(f"  Payload type: {type(payload).__name__}")
    if isinstance(payload, dict):
        print(f"  Payload keys: {list(payload.keys())[:10]}")
        sports = payload.get('sports', [])
        if isinstance(sports, list):
            print(f"  Sports: {len(sports)}")
            for j, sport in enumerate(sports[:2]):
                if isinstance(sport, dict):
                    comps = sport.get('competitions', [])
                    print(f"    Sport {j}: {len(comps)} competitions")
                    for k, comp in enumerate(comps[:2]):
                        if isinstance(comp, dict):
                            evts = comp.get('events', [])
                            print(f"      Comp {k} ({comp.get('name', '?')}): {len(evts)} events")
                            if len(evts) > 0:
                                e = evts[0]
                                print(f"        Event keys: {list(e.keys())[:15]}")
                                markets = e.get('markets', [])
                                print(f"        Markets: {len(markets)}")
                                if len(markets) > 0:
                                    m = markets[0]
                                    print(f"        Market keys: {list(m.keys())[:10]}")
                                    outcomes = m.get('outcomes', [])
                                    print(f"        Outcomes: {len(outcomes)}")
                                    for o in outcomes[:3]:
                                        print(f"          {o}")
