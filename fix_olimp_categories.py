"""
Fix OlimpBet Prematch - use categories endpoint to get events
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

# Step 1: Get sports with categories
url = "https://www.olimp.bet/api/v4/0/line/sports-with-categories-with-competitions?vids%5B%5D="
r = requests.get(url, headers=headers, timeout=15)
data = r.json()

print(f"Response items: {len(data)}")

total_events = 0
football_events = []

for item in data:
    payload = item.get('payload', {})
    if not isinstance(payload, dict):
        continue
    
    sport = payload.get('sport', {})
    sport_name = sport.get('name', '?')
    
    categories = payload.get('categoriesWithCompetitions', [])
    if not isinstance(categories, list):
        continue
    
    for cat in categories:
        if not isinstance(cat, dict):
            continue
        comps = cat.get('competitions', [])
        if not isinstance(comps, list):
            continue
        
        for comp in comps:
            if not isinstance(comp, dict):
                continue
            league_name = comp.get('name', '?')
            evts = comp.get('events', [])
            if isinstance(evts, list):
                total_events += len(evts)
                if 'футбол' in sport_name.lower() and len(evts) > 0:
                    for e in evts[:2]:
                        football_events.append({
                            'home': e.get('team1Name', ''),
                            'away': e.get('team2Name', ''),
                            'league': league_name,
                            'outcomes': len(e.get('outcomes', []))
                        })

print(f"\nTotal events across all categories: {total_events}")
print(f"Football events found: {len(football_events)}")

if football_events:
    print(f"\nFirst 5 football events:")
    for e in football_events[:5]:
        print(f"  {e['home']} vs {e['away']} ({e['league']}) - {e['outcomes']} outcomes")
