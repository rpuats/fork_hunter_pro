import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Get market definitions
url = "https://line-lb01-w.pb06e2-resources.com/line/factorsCatalog/sportBasicFactors?version=0&lang=ru&sysId=21"
headers = {"Accept": "application/json", "Referer": "https://pari.ru"}

r = requests.get(url, headers=headers, timeout=15)
data = r.json()

configs = data.get('result', {}).get('configs', [])
print(f"Configs: {len(configs)}")

# Find 1X2 market
for cfg in configs[:20]:
    name = cfg.get('name', '')
    factors = cfg.get('factors', [])
    if 'исход' in name.lower() or '1x2' in name.lower() or '1х2' in name.lower():
        print(f"\nMarket: {name}")
        print(f"  Factors: {len(factors)}")
        for f in factors[:5]:
            print(f"    id={f.get('id')}, name={f.get('name')}, shortName={f.get('shortName')}")

# Also look for outcome types
print("\n\nLooking for factor IDs around 921-925...")
for cfg in configs:
    factors = cfg.get('factors', [])
    for f in factors:
        fid = f.get('id', 0)
        if fid in [921, 922, 923, 924, 925, 1, 2, 3]:
            print(f"  id={fid}, name={f.get('name')}, shortName={f.get('shortName')}, market={cfg.get('name')}")
