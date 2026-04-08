"""
Fix OlimpBet Prematch - explore line structure deeper
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

# Get sports list first
url = "https://www.olimp.bet/api/v4/0/line/sports?sport=1"
r = requests.get(url, headers=headers, timeout=15)
data = r.json()

print(f"Sports: {len(data)}")
for item in data[:5]:
    payload = item.get('payload', {})
    sport = payload.get('sport', payload)
    print(f"  Sport: {sport.get('name', '?')}, eventCount={sport.get('eventCount', '?')}, id={payload.get('id', '?')}")

# Try to get events for football specifically
# Football sport ID might be different
print("\n\nTrying to get football events...")
# Try different sport IDs
for sport_id in [1, 4, 1001, 1004, 2001]:
    url = f"https://www.olimp.bet/api/v4/0/line/sports-with-categories-with-competitions?vids%5B%5D={sport_id}"
    try:
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            d = r.json()
            total = 0
            for item in d:
                payload = item.get('payload', {})
                cats = payload.get('categoriesWithCompetitions', [])
                for cat in cats:
                    comps = cat.get('competitions', [])
                    for comp in comps:
                        evts = comp.get('events', [])
                        total += len(evts) if isinstance(evts, list) else 0
            
            if total > 0:
                print(f"[OK] sport_id={sport_id}: {total} events")
    except:
        pass

# Also try the popular endpoint with more events
print("\n\nTrying popular with more events...")
url = "https://www.olimp.bet/api/v4/0/line/popular/sports-with-events?vids%5B%5D=&sportLimit=50&eventLimit=100"
r = requests.get(url, headers=headers, timeout=10)
if r.status_code == 200:
    d = r.json()
    total = 0
    for item in d:
        payload = item.get('payload', {})
        evts = payload.get('events', [])
        total += len(evts) if isinstance(evts, list) else 0
    print(f"Popular events: {total}")
