import requests
import sys
sys.stdout.reconfigure(encoding='utf-8')

# Test OlimpBet API endpoints
urls = [
    "https://service.olimp.bet/livefeed/v2?lng=ru&sport=1",
    "https://service.olimp.bet/prematchfeed/v2?lng=ru&sport=1",
    "https://service.olimp.bet/livefeed/v2?lng=ru",
    "https://service.olimp.bet/prematchfeed/v2?lng=ru",
]

headers = {
    "Accept": "application/json",
    "Referer": "https://www.olimp.bet",
    "Origin": "https://www.olimp.bet",
}

for url in urls:
    try:
        r = requests.get(url, headers=headers, timeout=10)
        print(f"[{r.status_code}] {url}")
        if r.status_code == 200:
            data = r.json()
            print(f"  Keys: {list(data.keys())[:10]}")
            events = data.get('events', []) or data.get('data', [])
            if isinstance(events, list):
                print(f"  Events: {len(events)}")
                if events:
                    e = events[0]
                    print(f"  First event keys: {list(e.keys())[:10]}")
    except Exception as ex:
        print(f"[ERR] {url}: {ex}")
