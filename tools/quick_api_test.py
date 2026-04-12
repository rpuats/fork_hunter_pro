"""Быстрый тест API для БК — проверяем типичные API эндпоинты"""
import requests
import json
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Accept": "application/json, text/plain, */*",
    "Accept-Language": "ru-RU,ru;q=0.9",
}

BK_ENDPOINTS = {
    "winline": [
        "https://winline.ru/api/line",
        "https://winline.ru/api/events",
        "https://winline.ru/api/v1/line",
        "https://line.winline.ru/api/line",
        "https://winline.ru/api/freebet/line",
        "https://api.winline.ru/v1/sport/line",
    ],
    "zenit": [
        "https://zenit.win/api/line",
        "https://zenit.win/api/events",
        "https://zenit.win/api/v1/line",
        "https://api.zenit.win/v1/sport/line",
        "https://zenit.win/line/football",
    ],
    "betcity": [
        "https://betcity.ru/api/line",
        "https://betcity.ru/api/events",
        "https://betcity.ru/api/v1/line",
        "https://api.betcity.ru/v1/sport/line",
        "https://betcity.ru/ru/line/football",
    ],
    "baltbet": [
        "https://baltbet.ru/api/line",
        "https://baltbet.ru/api/events",
        "https://baltbet.ru/api/v1/line",
        "https://api.baltbet.ru/v1/sport/line",
        "https://baltbet.ru/line",
    ],
}

def test_endpoint(bk, url):
    try:
        resp = requests.get(url, headers=HEADERS, timeout=10, verify=False)
        if resp.status_code == 200:
            try:
                data = resp.json()
                # Проверяем есть ли данные
                if isinstance(data, dict):
                    has_data = any(key in data for key in ['events', 'data', 'line', 'results', 'odds'])
                elif isinstance(data, list):
                    has_data = len(data) > 0
                else:
                    has_data = False
                
                return {
                    "bk": bk,
                    "url": url,
                    "status": resp.status_code,
                    "content_type": resp.headers.get("Content-Type", ""),
                    "size": len(resp.content),
                    "has_data": has_data,
                    "preview": str(data)[:300] if has_data else "",
                }
            except:
                return {
                    "bk": bk,
                    "url": url,
                    "status": resp.status_code,
                    "content_type": resp.headers.get("Content-Type", ""),
                    "size": len(resp.content),
                    "has_data": False,
                    "preview": resp.text[:200],
                }
        else:
            return None
    except Exception as e:
        return None

def main():
    print("🔍 Тестируем API эндпоинты для 4 БК...\n")
    
    all_urls = []
    for bk, urls in BK_ENDPOINTS.items():
        for url in urls:
            all_urls.append((bk, url))
    
    results = []
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = [executor.submit(test_endpoint, bk, url) for bk, url in all_urls]
        for future in as_completed(futures):
            result = future.result()
            if result:
                results.append(result)
    
    # Сортируем и выводим результаты
    print(f"\n📊 Найдено {len(results)} рабочих эндпоинтов:\n")
    
    for r in sorted(results, key=lambda x: x["size"], reverse=True):
        print(f"✅ {r['bk'].upper()}")
        print(f"   URL: {r['url']}")
        print(f"   Status: {r['status']}, Size: {r['size']:,} bytes")
        print(f"   Content-Type: {r['content_type']}")
        if r['has_data']:
            print(f"   🎯 HAS DATA!")
            print(f"   Preview: {r['preview'][:200]}")
        print()

if __name__ == "__main__":
    main()
