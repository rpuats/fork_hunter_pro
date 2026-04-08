"""
Find 1X2 factor IDs in Pari API
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://line-lb01-w.pb06e2-resources.com/line/factorsCatalog/tables?version=0&lang=ru&sysId=21"
headers = {"Accept": "application/json", "Referer": "https://pari.ru"}

r = requests.get(url, headers=headers, timeout=15)
data = r.json()

groups = data.get('groups', [])
print(f"Groups: {len(groups)}")

target_ids = [921, 922, 923, 924, 925, 926, 1, 2, 3]

for g in groups:
    gname = g.get('name', '')
    tables = g.get('tables', [])
    for t in tables:
        tname = t.get('name', '')
        rows = t.get('rows', [])
        for row in rows:
            for cell in row:
                fid = cell.get('factorId', 0)
                if fid in target_ids:
                    print(f"  Group: {gname}, Table: {tname}, Cell: {cell.get('name')}, factorId: {fid}")
