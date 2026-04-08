import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://www.olimp.bet/api/v4/0/live/sports-with-competitions-with-events?vids%5B%5D="
headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

r = requests.get(url, headers=headers, timeout=15)
data = r.json()

total_events = 0
for item in data:
    payload = item.get('payload', {})
    sport = payload.get('sport', {})
    sport_name = sport.get('name', '?')
    comps = payload.get('competitionsWithEvents', [])
    
    for comp in comps:
        events = comp.get('events', [])
        total_events += len(events)
        
        if len(events) > 0 and total_events <= 5:
            e = events[0]
            print(f"\nSport: {sport_name}")
            print(f"  Comp: {comp.get('name', '?')}")
            print(f"  Event keys: {list(e.keys())[:20]}")
            print(f"  Event: {e.get('name1')} vs {e.get('name2')}")
            markets = e.get('markets', [])
            print(f"  Markets: {len(markets)}")
            if len(markets) > 0:
                m = markets[0]
                print(f"  Market: {m.get('name')}")
                outcomes = m.get('outcomes', [])
                print(f"  Outcomes: {len(outcomes)}")
                for o in outcomes[:3]:
                    print(f"    {o}")

print(f"\nTotal events across all sports: {total_events}")
