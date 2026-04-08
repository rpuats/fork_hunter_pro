import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://www.olimp.bet", "User-Agent": "Mozilla/5.0"}

# Test the new TOP endpoint for line
url = "https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D="
print(f"Testing: {url}")

r = requests.get(url, headers=headers, timeout=15)
print(f"Status: {r.status_code}")

if r.status_code == 200:
    try:
        data = r.json()
        print(f"Type: {type(data).__name__}")
        
        if isinstance(data, list):
            print(f"Items: {len(data)}")
            
            total_events = 0
            football_events = []
            
            for item in data:
                payload = item.get('payload', {})
                if not isinstance(payload, dict):
                    continue
                
                sport = payload.get('sport', {})
                sport_name = sport.get('name', '?')
                
                comps = payload.get('competitionsWithEvents', [])
                if not isinstance(comps, list):
                    continue
                
                for comp in comps:
                    if not isinstance(comp, dict):
                        continue
                    league = comp.get('name', '?')
                    evts = comp.get('events', [])
                    if isinstance(evts, list):
                        total_events += len(evts)
                        if 'футбол' in sport_name.lower() and len(evts) > 0:
                            for e in evts[:2]:
                                football_events.append({
                                    'home': e.get('team1Name', ''),
                                    'away': e.get('team2Name', ''),
                                    'league': league,
                                    'outcomes': len(e.get('outcomes', []))
                                })
            
            print(f"\nTotal events: {total_events}")
            print(f"Football events sample: {len(football_events)}")
            
            for e in football_events[:5]:
                print(f"  {e['home']} vs {e['away']} ({e['league']}) - {e['outcomes']} outcomes")
        
        elif isinstance(data, dict):
            print(f"Keys: {list(data.keys())[:10]}")
    except Exception as e:
        print(f"Parse error: {e}")
        print(f"Response: {r.text[:500]}")
else:
    print(f"Response: {r.text[:300]}")
