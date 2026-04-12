import requests
import json

# Test the catalog endpoint
url = "https://winline.ru/api/v2/catalog?country=ru"
headers = {
    'Accept': 'application/json, text/plain, */*',
    'Accept-Language': 'ru-RU,ru;q=0.9',
    'language': 'ru-RU',
    'Referer': 'https://winline.ru/football',
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
}

try:
    response = requests.get(url, headers=headers, timeout=15)
    print(f"Status: {response.status_code}")
    print(f"Content-Type: {response.headers.get('content-type')}")

    if response.status_code == 200:
        if 'json' in response.headers.get('content-type', ''):
            data = response.json()
            print("JSON parsed successfully")
            print(f"Size: {len(json.dumps(data))}")

            # Look for structure
            if isinstance(data, dict):
                print(f"Keys: {list(data.keys())}")

                # Check for nested IDs
                def find_ids(obj, path=""):
                    if isinstance(obj, dict):
                        for k, v in obj.items():
                            if k == 'id' and isinstance(v, (int, str)):
                                print(f"ID at {path}.{k}: {v}")
                            find_ids(v, f"{path}.{k}" if path else k)
                    elif isinstance(obj, list):
                        for i, item in enumerate(obj):
                            find_ids(item, f"{path}[{i}]")

                find_ids(data)
            else:
                print(f"Root type: {type(data)}")
        else:
            print("Not JSON content")
            print(response.text[:200])
    else:
        print(f"HTTP error: {response.status_code}")

except Exception as e:
    print(f"Error: {e}")