import requests
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://www.olimp.bet/api/v4/0/line/sports-with-competitions-with-events?vids%5B%5D="
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
        evts = comp.get('events', [])
        total_events += len(evts)

print(f"OlimpBet prematch: {total_events} events from {len(data)} sports")
