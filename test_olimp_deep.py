"""
Explore OlimpBet API payload structure
"""
import requests
import json
import sys
sys.stdout.reconfigure(encoding='utf-8')

url = "https://www.olimp.bet/api/v4/0/live/sports-with-competitions-with-events?vids%5B%5D="
headers = {
    "Accept": "application/json",
    "Referer": "https://www.olimp.bet",
    "User-Agent": "Mozilla/5.0",
}

r = requests.get(url, headers=headers, timeout=15)
data = r.json()

# Each item has: operationId, version, payload
for item in data[:3]:
    op_id = item.get('operationId')
    payload = item.get('payload')
    print(f"\nOperation: {op_id}")
    print(f"  Payload type: {type(payload).__name__}")
    if isinstance(payload, dict):
        print(f"  Payload keys: {list(payload.keys())[:10]}")
        # Look for events
        for key in payload:
            val = payload[key]
            if isinstance(val, list) and len(val) > 0:
                print(f"  {key}: list[{len(val)}]")
                first = val[0]
                if isinstance(first, dict):
                    print(f"    First keys: {list(first.keys())[:15]}")
                    # Look for events inside
                    for k2 in first:
                        v2 = first[k2]
                        if isinstance(v2, list) and len(v2) > 0:
                            print(f"    {k2}: list[{len(v2)}]")
                            if isinstance(v2[0], dict):
                                print(f"      First keys: {list(v2[0].keys())[:15]}")
                                # Look for odds
                                for k3 in v2[0]:
                                    v3 = v2[0][k3]
                                    if isinstance(v3, list) and len(v3) > 0:
                                        print(f"      {k3}: list[{len(v3)}]")
                                        if isinstance(v3[0], dict):
                                            print(f"        First keys: {list(v3[0].keys())[:10]}")
    elif isinstance(payload, list):
        print(f"  Payload: list[{len(payload)}]")
