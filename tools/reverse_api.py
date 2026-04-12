"""Поиск реальных API БК через анализ JS и сетевых запросов"""
import requests
import re
import json
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "ru-RU,ru;q=0.9,en;q=0.8",
}

def find_api_endpoints(bk_name, base_url, known_patterns=None):
    """Ищем API эндпоинты через анализ HTML и JS"""
    print(f"\n🔍 {bk_name}: анализ {base_url}")
    
    results = {bk_name: []}
    
    try:
        # 1. Загружаем главную страницу
        resp = requests.get(base_url, headers=HEADERS, timeout=15, allow_redirects=True)
        html = resp.text
        
        # Ищем script src
        scripts = re.findall(r'src=["\']([^"\']*\.js[^"\']*)["\']', html)
        print(f"  Найдено {len(scripts)} скриптов")
        
        # 2. Проверяем известные API паттерны
        api_tests = [
            f"{base_url.rstrip('/')}/api/v1/line",
            f"{base_url.rstrip('/')}/api/v2/line",
            f"{base_url.rstrip('/')}/api/line",
            f"{base_url.rstrip('/')}/v1/line",
            f"{base_url.rstrip('/')}/line/api",
            f"{base_url.rstrip('/')}/events",
        ]
        
        if known_patterns:
            api_tests.extend(known_patterns)
        
        # 3. Тестируем каждый API
        for api_url in set(api_tests):
            try:
                r = requests.get(api_url, headers={
                    **HEADERS,
                    "Accept": "application/json",
                    "X-Requested-With": "XMLHttpRequest",
                }, timeout=8)
                
                if r.status_code == 200:
                    ct = r.headers.get("Content-Type", "")
                    if "json" in ct.lower():
                        try:
                            data = r.json()
                            size = len(r.content)
                            print(f"  ✅ JSON API: {api_url} ({size:,} bytes)")
                            if isinstance(data, dict):
                                print(f"     Keys: {list(data.keys())[:5]}")
                            elif isinstance(data, list):
                                print(f"     Array[{len(data)}]")
                            results[bk_name].append({
                                "url": api_url,
                                "type": "json",
                                "size": size,
                                "sample": str(data)[:200]
                            })
                        except:
                            pass
                    else:
                        # Может быть HTML но с данными
                        if len(r.content) > 1000:
                            print(f"  ⚠️  HTML API: {api_url} ({len(r.content):,} bytes)")
                            results[bk_name].append({
                                "url": api_url,
                                "type": "html",
                                "size": len(r.content),
                            })
            except Exception as e:
                pass
        
        # 4. Проверяем CDN/статические ресурсы
        cdn_patterns = [
            f"https://{bk_name.lower()}.ru/static/",
            f"https://cdn.{bk_name.lower()}.ru/",
            f"https://api.{bk_name.lower()}.ru/",
        ]
        
    except Exception as e:
        print(f"  ❌ Ошибка: {e}")
    
    return results

def search_winline():
    """Специфичный поиск для Winline"""
    print("\n🎯 WINLINE — глубокий поиск API...")
    
    # Winline известен своими API
    urls_to_test = [
        "https://winline.ru/api/line",
        "https://winline.ru/api/v1/line",
        "https://winline.ru/api/v2/line",
        "https://winline.ru/api/v1/events",
        "https://winline.ru/api/v2/events",
        "https://api.winline.ru/v1/line",
        "https://api.winline.ru/v2/line",
        "https://winline.ru/betting-api/line",
        "https://winline.ru/graphql",
        # Известные эндпоинты Winline
        "https://winline.ru/api/freebet/line",
        "https://winline.ru/api/sport/1/line",
    ]
    
    found = []
    for url in urls_to_test:
        try:
            r = requests.get(url, headers={
                **HEADERS,
                "Accept": "application/json",
                "Referer": "https://winline.ru/",
                "Origin": "https://winline.ru",
            }, timeout=8)
            
            if r.status_code == 200:
                ct = r.headers.get("Content-Type", "")
                if "json" in ct.lower():
                    data = r.json()
                    size = len(r.content)
                    print(f"  ✅ {url} → {size:,} bytes")
                    found.append({"url": url, "size": size, "data": str(data)[:300]})
                else:
                    print(f"  ⚠️  {url} → HTML ({len(r.content):,} bytes)")
            else:
                print(f"  ❌ {url} → {r.status_code}")
        except Exception as e:
            print(f"  ❌ {url} → {e}")
    
    return found

def main():
    print("=" * 80)
    print("🔍 ГЛУБОКИЙ ПОИСК API ДЛЯ БК")
    print("=" * 80)
    
    # 1. Winline
    winline_results = search_winline()
    
    # 2. Zenit
    print("\n🎯 ZENIT — поиск API...")
    zenit_urls = [
        "https://zenit.win/api/line",
        "https://zenit.win/api/v1/line",
        "https://zenit.win/api/v2/line",
        "https://zenit.win/api/events",
        "https://api.zenit.win/v1/line",
        "https://api.zenit.win/v2/line",
        "https://zenit.win/bets/api/line",
    ]
    for url in zenit_urls:
        try:
            r = requests.get(url, headers={**HEADERS, "Accept": "application/json"}, timeout=8)
            if r.status_code == 200:
                ct = r.headers.get("Content-Type", "")
                if "json" in ct.lower():
                    print(f"  ✅ {url} → {len(r.content):,} bytes JSON")
                else:
                    print(f"  ⚠️  {url} → HTML ({len(r.content):,} bytes)")
            else:
                print(f"  ❌ {url} → {r.status_code}")
        except Exception as e:
            print(f"  ❌ {url} → {e}")
    
    # 3. Betcity
    print("\n🎯 BETCITY — поиск API...")
    betcity_urls = [
        "https://betcity.ru/api/line",
        "https://betcity.ru/api/v1/line",
        "https://betcity.ru/api/v2/line",
        "https://betcity.ru/api/events",
        "https://api.betcity.ru/v1/line",
        "https://betcity.ru/line/api",
        "https://betcity.ru/graphql",
    ]
    for url in betcity_urls:
        try:
            r = requests.get(url, headers={**HEADERS, "Accept": "application/json"}, timeout=8)
            if r.status_code == 200:
                ct = r.headers.get("Content-Type", "")
                if "json" in ct.lower():
                    print(f"  ✅ {url} → {len(r.content):,} bytes JSON")
                else:
                    print(f"  ⚠️  {url} → HTML ({len(r.content):,} bytes)")
            else:
                print(f"  ❌ {url} → {r.status_code}")
        except Exception as e:
            print(f"  ❌ {url} → {e}")
    
    # 4. Baltbet
    print("\n🎯 BALTBET — поиск API...")
    baltbet_urls = [
        "https://baltbet.ru/api/line",
        "https://baltbet.ru/api/v1/line",
        "https://baltbet.ru/api/v2/line",
        "https://baltbet.ru/api/events",
        "https://api.baltbet.ru/v1/line",
        "https://baltbet.ru/line/api",
    ]
    for url in baltbet_urls:
        try:
            r = requests.get(url, headers={**HEADERS, "Accept": "application/json"}, timeout=8)
            if r.status_code == 200:
                ct = r.headers.get("Content-Type", "")
                if "json" in ct.lower():
                    print(f"  ✅ {url} → {len(r.content):,} bytes JSON")
                else:
                    print(f"  ⚠️  {url} → HTML ({len(r.content):,} bytes)")
            else:
                print(f"  ❌ {url} → {r.status_code}")
        except Exception as e:
            print(f"  ❌ {url} → {e}")
    
    # Сохраняем результаты
    all_results = {
        "winline": winline_results,
    }
    
    with open("api_discovery_results.json", "w", encoding="utf-8") as f:
        json.dump(all_results, f, indent=2, ensure_ascii=False)
    
    print(f"\n📊 Результаты сохранены в api_discovery_results.json")

if __name__ == "__main__":
    main()
