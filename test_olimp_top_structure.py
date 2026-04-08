import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

url = "https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D="
r = requests.get(url, headers=headers, timeout=15)
data = r.json()

for item in data[:3]:
    payload = item.get('payload', {})
    sport = payload.get('sport', {})
    sport_name = sport.get('name', '?')
    
    comps = payload.get('competitionsWithEvents', [])
    for comp in comps[:2]:
        league = comp.get('name', '?')
        league_id = comp.get('id', '?')
        evts = comp.get('events', [])
        if isinstance(evts, list) and len(evts) > 0:
            e = evts[0]
            print(f"\nSport: {sport_name}")
            print(f"  League: {league} (id={league_id})")
            print(f"  Event: {e.get('team1Name')} vs {e.get('team2Name')}")
            print(f"  Event keys: {list(e.keys())[:15]}")
            
            outcomes = e.get('outcomes', [])
            print(f"  Outcomes: {len(outcomes)}")
            for o in outcomes[:5]:
                print(f"    shortName={o.get('shortName')}, odds={o.get('probability')}, marketId={o.get('marketId')}, groupName={o.get('groupName')}")
