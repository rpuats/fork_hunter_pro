import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Get tables to find 1X2 factor mapping
url = "https://line-lb01-w.pb06e2-resources.com/line/factorsCatalog/tables?version=0&lang=ru&sysId=21"
headers = {"Accept": "application/json", "Referer": "https://pari.ru"}

r = requests.get(url, headers=headers, timeout=15)
data = r.json()

groups = data.get('result', {}).get('groups', [])
print(f"Groups: {len(groups)}")

for g in groups[:5]:
    name = g.get('name', '')
    tables = g.get('tables', [])
    if 'исход' in name.lower() or 'result' in name.lower():
        print(f"\nGroup: {name}")
        for t in tables[:3]:
            tname = t.get('name', '')
            outcomes = t.get('outcomes', [])
            print(f"  Table: {tname}")
            for o in outcomes[:5]:
                print(f"    id={o.get('id')}, name={o.get('name')}, shortName={o.get('shortName')}")
