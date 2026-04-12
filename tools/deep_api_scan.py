"""Глубокий анализ API — ищем реальные JSON API через анализ HTML и типичных паттернов"""
import requests
import re
import json

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Accept": "*/*",
}

def analyze_winline():
    print("\n🔍 WINLINE — глубокий анализ...")
    
    # Пробуем разные известные API паттерны
    urls_to_try = [
        # Winline использует cdn для данных
        "https://winline.ru/api/v2/sports",
        "https://winline.ru/api/v1/sports",
        "https://winline.ru/api/v2/line/prematch",
        "https://winline.ru/api/v1/line/prematch",
        
        # Типичные для Winline API
        "https://winline.ru/api/line/v2/sport/4",
        "https://winline.ru/api/line/v2/sport/1",
        
        # Статические данные
        "https://winline.ru/static/line.json",
        "https://winline.ru/static/api/line.json",
    ]
    
    for url in urls_to_try:
        try:
            resp = requests.get(url, headers=HEADERS, timeout=10)
            ct = resp.headers.get("Content-Type", "")
            if "json" in ct.lower():
                try:
                    data = resp.json()
                    print(f"  ✅ JSON: {url}")
                    print(f"     Size: {len(resp.content):,}")
                    print(f"     Keys: {list(data.keys())[:10] if isinstance(data, dict) else f'Array[{len(data)}]'}")
                except:
                    pass
        except:
            pass

def analyze_zenit():
    print("\n🔍 ZENIT — глубокий анализ...")
    
    urls_to_try = [
        "https://zenit.win/api/v2/sports",
        "https://zenit.win/api/v1/sports",
        "https://zenit.win/api/v2/line",
        "https://zenit.win/api/v1/line",
        "https://zenit.win/api/line/v2/sport/4",
        "https://zenit.win/api/line/v2/sport/1",
        "https://zenit.win/static/line.json",
    ]
    
    for url in urls_to_try:
        try:
            resp = requests.get(url, headers=HEADERS, timeout=10)
            ct = resp.headers.get("Content-Type", "")
            if "json" in ct.lower():
                try:
                    data = resp.json()
                    print(f"  ✅ JSON: {url}")
                    print(f"     Size: {len(resp.content):,}")
                    print(f"     Keys: {list(data.keys())[:10] if isinstance(data, dict) else f'Array[{len(data)}]'}")
                except:
                    pass
        except:
            pass

def analyze_betcity():
    print("\n🔍 BETCITY — глубокий анализ...")
    
    urls_to_try = [
        "https://betcity.ru/api/v2/sports",
        "https://betcity.ru/api/v1/sports",
        "https://betcity.ru/api/v2/line",
        "https://betcity.ru/api/v1/line",
        "https://betcity.ru/api/line/v2/sport/4",
        "https://betcity.ru/api/line/v2/sport/1",
        "https://betcity.ru/static/line.json",
    ]
    
    for url in urls_to_try:
        try:
            resp = requests.get(url, headers=HEADERS, timeout=10)
            ct = resp.headers.get("Content-Type", "")
            if "json" in ct.lower():
                try:
                    data = resp.json()
                    print(f"  ✅ JSON: {url}")
                    print(f"     Size: {len(resp.content):,}")
                    print(f"     Keys: {list(data.keys())[:10] if isinstance(data, dict) else f'Array[{len(data)}]'}")
                except:
                    pass
        except:
            pass

def analyze_baltbet():
    print("\n🔍 BALTBET — глубокий анализ...")
    
    urls_to_try = [
        "https://baltbet.ru/api/v2/sports",
        "https://baltbet.ru/api/v1/sports",
        "https://baltbet.ru/api/v2/line",
        "https://baltbet.ru/api/v1/line",
        "https://baltbet.ru/api/line/v2/sport/4",
        "https://baltbet.ru/api/line/v2/sport/1",
        "https://baltbet.ru/static/line.json",
    ]
    
    for url in urls_to_try:
        try:
            resp = requests.get(url, headers=HEADERS, timeout=10)
            ct = resp.headers.get("Content-Type", "")
            if "json" in ct.lower():
                try:
                    data = resp.json()
                    print(f"  ✅ JSON: {url}")
                    print(f"     Size: {len(resp.content):,}")
                    print(f"     Keys: {list(data.keys())[:10] if isinstance(data, dict) else f'Array[{len(data)}]'}")
                except:
                    pass
        except:
            pass

if __name__ == "__main__":
    analyze_winline()
    analyze_zenit()
    analyze_betcity()
    analyze_baltbet()
    print("\n✅ Готово!")
