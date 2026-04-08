"""
Explore Pari/Bettery/Fonbet shared API structure
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Pari API
url = "https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300"
headers = {"Accept": "application/json", "Referer": "https://pari.ru"}

print("Fetching Pari events/listBase...")
r = requests.get(url, headers=headers, timeout=30)
data = r.json()

print(f"Keys: {list(data.keys())}")
print(f"packetVersion: {data.get('packetVersion')}")

# The response is huge - let's look at structure
# It should have events, competitions, sports etc.
for key in data:
    val = data[key]
    if isinstance(val, dict):
        print(f"\n{key}: dict with keys {list(val.keys())[:10]}")
    elif isinstance(val, list):
        print(f"\n{key}: list[{len(val)}]")
        if len(val) > 0:
            first = val[0]
            if isinstance(first, dict):
                print(f"  First keys: {list(first.keys())[:15]}")
    elif isinstance(val, (int, str, float)):
        print(f"\n{key}: {val}")
