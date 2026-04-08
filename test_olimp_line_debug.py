import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Test OlimpBet line API
url = "https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D="
headers = {
    "Accept": "application/json",
    "Referer": "https://www.olimp.bet",
    "Origin": "https://www.olimp.bet",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
}

r = requests.get(url, headers=headers, timeout=15)
print(f"Status: {r.status_code}")
print(f"Content-Type: {r.headers.get('content-type')}")
print(f"Content length: {len(r.content)}")
print(f"Text length: {len(r.text)}")
print(f"First 200 chars: {r.text[:200]}")

try:
    data = r.json()
    print(f"\nParsed OK! Type: {type(data).__name__}")
    if isinstance(data, list):
        print(f"Items: {len(data)}")
except Exception as e:
    print(f"\nJSON parse error: {e}")
