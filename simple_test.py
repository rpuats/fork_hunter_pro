import requests
import json

url = "https://winline.ru/api/static-data/alter/1/80632"
headers = {
    'Accept': 'application/json',
    'Referer': 'https://winline.ru',
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
}

try:
    response = requests.get(url, headers=headers, timeout=15)
    print(f"Status: {response.status_code}")
    print(f"Content-Type: {response.headers.get('content-type')}")

    if response.status_code == 200:
        try:
            data = response.json()
            print(f"JSON parsed successfully, size: {len(json.dumps(data))}")
            print(f"Keys: {list(data.keys())}")
        except:
            print("Failed to parse JSON")
            print(f"First 200 chars: {response.text[:200]}")
except Exception as e:
    print(f"Error: {e}")