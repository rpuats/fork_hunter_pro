import requests
import sys
import json
sys.stdout.reconfigure(encoding='utf-8')

# Leon API endpoints for full event list
base = "https://leon.ru/api-2/betline"

urls = [
    # Live events
    f"{base}/events/inplay?ctag=ru-RU&sport=1&flags=reg,urlv2,orn2,mm2,rrc",
    f"{base}/events/inplayupcomingall?ctag=ru-RU&sport=1&flags=reg,urlv2,orn2,mm2,rrc",
    f"{base}/events/inplay?ctag=ru-RU&flags=reg,urlv2,orn2,mm2,rrc",
    # Prematch events  
    f"{base}/events/prematch?ctag=ru-RU&sport=1&flags=reg,urlv2,orn2,mm2,rrc",
    f"{base}/events/prematch?ctag=ru-RU&flags=reg,urlv2,orn2,mm2,rrc",
    # All events
    f"{base}/events?ctag=ru-RU&sport=1&flags=reg,urlv2,orn2,mm2,rrc",
    f"{base}/events?ctag=ru-RU&flags=reg,urlv2,orn2,mm2,rrc",
    # Alternative
    f"{base}/events/inplay?ctag=ru-RU",
    f"{base}/events/prematch?ctag=ru-RU",
]

headers = {
    "Accept": "application/json",
    "Referer": "https://leon.ru",
    "Origin": "https://leon.ru",
}

for url in urls:
    try:
        r = requests.get(url, headers=headers, timeout=15)
        if r.status_code == 200:
            data = r.json()
            events = data.get('events', [])
            if isinstance(events, dict):
                events = events.get('events', [])
            count = len(events) if isinstance(events, list) else 'N/A'
            print(f"[OK] {count} events - {url[:80]}")
            if isinstance(events, list) and len(events) > 0:
                e = events[0]
                print(f"     Keys: {list(e.keys())[:8]}")
        else:
            print(f"[{r.status_code}] {url[:80]}")
    except Exception as ex:
        print(f"[ERR] {url[:80]}: {type(ex).__name__}")
