import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Test MarathonBet API endpoints
headers = {"Accept": "application/json", "Referer": "https://www.marathonbet.ru", "User-Agent": "Mozilla/5.0"}

# 1. Test the line API (same platform as Pari/Bettery)
url1 = "https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket=3000"
print(f"Testing line API (scopeMarket=3000)...")
try:
    r = requests.get(url1, headers=headers, timeout=15)
    print(f"  Status: {r.status_code}")
    if r.status_code == 200:
        try:
            data = r.json()
            events = data.get('events', [])
            print(f"  Events: {len(events)}")
            if events:
                e = events[0]
                print(f"  First event keys: {list(e.keys())[:10]}")
                print(f"  Sample: {e.get('team1')} vs {e.get('team2')}")
                
                # Check for customFactors
                factors = data.get('customFactors', [])
                print(f"  Custom factors: {len(factors)}")
        except Exception as e:
            print(f"  JSON error: {e}")
            print(f"  First 200 chars: {r.text[:200]}")
except Exception as e:
    print(f"  Error: {e}")

# 2. Test live update API (JSONP)
url2 = "https://lu.marathonbet.ru/su/liveupdate/popular/?callback=liveUpdate&markets=&expandedSportIds=26418,22723&siteStyle=MULTIMARKETS&timeZone=Europe/Moscow&oddsType=Decimal"
print(f"\nTesting live update API (JSONP)...")
try:
    r = requests.get(url2, headers=headers, timeout=15)
    print(f"  Status: {r.status_code}")
    if r.status_code == 200:
        # JSONP - strip callback wrapper
        text = r.text
        if text.startswith('liveUpdate(') and text.endswith(')'):
            text = text[11:-1]
        try:
            data = json.loads(text)
            print(f"  Keys: {list(data.keys())[:10] if isinstance(data, dict) else 'not dict'}")
            if isinstance(data, dict):
                for k, v in data.items():
                    if isinstance(v, list):
                        print(f"  {k}: list[{len(v)}]")
        except Exception as e:
            print(f"  JSON error: {e}")
            print(f"  First 300 chars: {text[:300]}")
except Exception as e:
    print(f"  Error: {e}")

# 3. Try different scopeMarket values
print("\n\nTrying different scopeMarket values...")
for sm in [3000, 3001, 1600, 2300, 501]:
    url = f"https://line51.tf39be-resources.com/events/listBase?lang=ru&scopeMarket={sm}"
    try:
        r = requests.get(url, headers=headers, timeout=10)
        if r.status_code == 200:
            try:
                d = r.json()
                evts = d.get('events', [])
                print(f"  scopeMarket={sm}: {len(evts)} events")
            except:
                print(f"  scopeMarket={sm}: not JSON")
        else:
            print(f"  scopeMarket={sm}: {r.status_code}")
    except Exception as e:
        print(f"  scopeMarket={sm}: {type(e).__name__}")
