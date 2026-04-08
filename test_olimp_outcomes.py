import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://www.olimp.bet/api/v4/0/live/sports-with-competitions-with-events?vids%5B%5D="
headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

r = requests.get(url, headers=headers, timeout=15)
data = r.json()

for item in data:
    payload = item.get('payload', {})
    sport = payload.get('sport', {})
    sport_name = sport.get('name', '?')
    if 'футбол' not in sport_name.lower():
        continue
    
    comps = payload.get('competitionsWithEvents', [])
    for comp in comps:
        evts = comp.get('events', [])
        for evt in evts[:2]:
            home = evt.get('team1Name', '')
            away = evt.get('team2Name', '')
            outcomes = evt.get('outcomes', [])
            print(f"\n{home} vs {away}")
            print(f"  Outcomes: {len(outcomes)}")
            for o in outcomes[:5]:
                print(f"    {o}")
            break
        break
    break
