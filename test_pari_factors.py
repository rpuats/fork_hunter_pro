import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300"
headers = {"Accept": "application/json", "Referer": "https://pari.ru"}

r = requests.get(url, headers=headers, timeout=30)
data = r.json()

events = data.get('events', [])
custom_factors = data.get('customFactors', [])
sports = data.get('sports', [])

# Build sport lookup
sport_map = {s['id']: s['name'] for s in sports}

# Look at first football event
football_events = [e for e in events if e.get('rootKind') == 1 or e.get('kind') == 1]
print(f"Total events: {len(events)}")
print(f"Football events: {len(football_events)}")
print(f"Custom factors: {len(custom_factors)}")

if football_events:
    evt = football_events[0]
    print(f"\nFirst football event:")
    print(f"  id={evt.get('id')}, team1={evt.get('team1')}, team2={evt.get('team2')}")
    print(f"  sportId={evt.get('sportId')}, kind={evt.get('kind')}, rootKind={evt.get('rootKind')}")
    print(f"  startTime={evt.get('startTime')}")
    
    # Find factors for this event
    evt_id = evt.get('id')
    factors = [f for f in custom_factors if f.get('e') == evt_id]
    print(f"\n  Factors for this event: {len(factors)}")
    
    if factors:
        f = factors[0]
        print(f"  First factor keys: {list(f.keys())}")
        print(f"  countAll: {f.get('countAll')}")
        factor_list = f.get('factors', [])
        print(f"  factors list length: {len(factor_list)}")
        if factor_list:
            for fac in factor_list[:5]:
                print(f"    Factor: {fac}")
