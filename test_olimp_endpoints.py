import requests
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Test OlimpBet alternative endpoints
urls = [
    "https://www.olimp.bet/live/feed/1",
    "https://olimp.bet/api/events/live",
    "https://olimp.bet/api/v1/events",
    "https://api.olimp.bet/events/live",
    "https://www.olimp.bet/api/live",
    "https://olimp.bet/live",
]

headers = {
    "Accept": "application/json",
    "Referer": "https://www.olimp.bet",
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
}

for url in urls:
    try:
        r = requests.get(url, headers=headers, timeout=10)
        print(f"[{r.status_code}] {url[:60]}")
        if r.status_code == 200:
            try:
                data = r.json()
                if isinstance(data, dict):
                    print(f"  Keys: {list(data.keys())[:5]}")
                elif isinstance(data, list):
                    print(f"  List with {len(data)} items")
            except:
                print(f"  Not JSON, length: {len(r.text)}")
    except Exception as ex:
        print(f"[ERR] {url[:60]}: {type(ex).__name__}")
