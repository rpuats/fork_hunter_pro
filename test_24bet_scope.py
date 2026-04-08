import requests
import sys
sys.stdout.reconfigure(encoding='utf-8')

headers = {"Accept": "application/json", "Referer": "https://24bet.ru", "User-Agent": "Mozilla/5.0"}

# 24bet uses same platform as MarathonBet but different scopeMarket
# Try different scopeMarket values
for sm in [501, 1600, 2300, 3000, 3001, 4000, 5000, 6000, 7000, 8000, 9000]:
    url = f"https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket={sm}"
    try:
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            try:
                d = r.json()
                evts = d.get('events', [])
                factors = d.get('customFactors', [])
                if len(evts) > 100:
                    print(f"✅ scopeMarket={sm}: {len(evts)} events, {len(factors)} factors")
                    if evts:
                        e = evts[0]
                        print(f"   Sample: {e.get('team1')} vs {e.get('team2')}")
            except:
                pass
        else:
            print(f"   scopeMarket={sm}: {r.status_code}")
    except Exception as e:
        print(f"   scopeMarket={sm}: {type(e).__name__}")

# Also try different domains for 24bet
print("\n\nTrying different domains...")
domains = [
    "line51.tf39be-resources.com",
    "line-lb01-w.pb06e2-resources.com",  # Pari
    "line01.at58f5-resources.com",  # Bettery
]

for domain in domains:
    url = f"https://{domain}/events/listBase?lang=ru&scopeMarket=501"
    try:
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            try:
                d = r.json()
                evts = d.get('events', [])
                if len(evts) > 100:
                    print(f"✅ {domain}/scopeMarket=501: {len(evts)} events")
            except:
                pass
    except:
        pass
