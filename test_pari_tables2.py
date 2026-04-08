import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://line-lb01-w.pb06e2-resources.com/line/factorsCatalog/tables?version=0&lang=ru&sysId=21"
headers = {"Accept": "application/json", "Referer": "https://pari.ru"}

r = requests.get(url, headers=headers, timeout=15)
print(f"Status: {r.status_code}")
print(f"Content-Type: {r.headers.get('content-type')}")
print(f"First 500 chars: {r.text[:500]}")
