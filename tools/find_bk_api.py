"""Поиск API эндпоинтов БК через анализ JS-файлов сайтов"""
import requests
import re
import json

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
}

def find_js_files(url):
    """Ищем JS файлы на странице"""
    try:
        resp = requests.get(url, headers=HEADERS, timeout=15)
        # Ищем script src="*.js"
        scripts = re.findall(r'src=["\']([^"\']*\.js[^"\']*)["\']', resp.text)
        return scripts
    except Exception as e:
        print(f"  Error: {e}")
        return []

def search_api_patterns(js_url):
    """Ищем API паттерны в JS файле"""
    try:
        resp = requests.get(js_url, headers=HEADERS, timeout=10)
        text = resp.text
        
        # Паттерны API
        patterns = [
            r'["\'](/api/[^"\']+)["\']',
            r'["\'](/v\d+/[^"\']+)["\']',
            r'["\'](https?://[^"\']*api[^"\']+)["\']',
            r'["\'](https?://[^"\']*line[^"\']+)["\']',
            r'["\'](https?://[^"\']*odds[^"\']+)["\']',
            r'["\'](https?://[^"\']*events[^"\']+)["\']',
            r'["\'](https?://[^"\']*factors[^"\']+)["\']',
        ]
        
        found = set()
        for pattern in patterns:
            matches = re.findall(pattern, text, re.IGNORECASE)
            for m in matches:
                if len(m) > 10 and 'google' not in m and 'analytics' not in m:
                    found.add(m)
        
        return list(found)[:10]  # Max 10
    except:
        return []

def analyze_bk(name, main_url):
    print(f"\n🔍 {name.upper()} — анализ...")
    
    # Ищем JS файлы
    js_files = find_js_files(main_url)
    print(f"  Найдено {len(js_files)} JS файлов")
    
    all_apis = []
    for js in js_files[:15]:  # Проверяем первые 15
        if not js.startswith('http'):
            if js.startswith('/'):
                js = f"https://{name.lower()}.{['ru','win'][name.lower() in ['zenit','baltbet']]}" + js
            else:
                js = main_url.rstrip('/') + '/' + js
        
        apis = search_api_patterns(js)
        if apis:
            print(f"  ✅ {js}")
            for api in apis[:5]:
                print(f"     → {api}")
                all_apis.append(api)
    
    return all_apis

def main():
    bks = {
        "Winline": "https://winline.ru/football",
        "Zenit": "https://zenit.win/line/football",
        "Betcity": "https://betcity.ru/ru/line/football",
        "Baltbet": "https://baltbet.ru/line",
    }
    
    all_results = {}
    for name, url in bks.items():
        apis = analyze_bk(name, url)
        all_results[name] = apis
    
    print("\n" + "="*80)
    print("📊 ИТОГИ:")
    for bk, apis in all_results.items():
        print(f"\n{bk}: {len(apis)} API эндпоинтов")
        for api in apis:
            print(f"  - {api}")

if __name__ == "__main__":
    main()
