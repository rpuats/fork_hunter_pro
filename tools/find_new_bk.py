"""Поиск новых БК без Cloudflare защиты"""
import requests
import json
import sys

# Потенциальные БК для проверки
BK_CANDIDATES = [
    # Российские
    ("olimpbet", "https://olimp.bet"),
    ("ligastavok", "https://ligastavok.ru"),
    ("parimatch", "https://parimatch.ru"),
    ("fonbet", "https://fonbet.ru"),  # Уже работает
    ("winline", "https://winline.ru"),  # Cloudflare
    ("zenit", "https://zenit.win"),  # Cloudflare
    ("betcity", "https://betcity.ru"),  # Cloudflare
    ("baltbet", "https://baltbet.ru"),  # Cloudflare
    # Международные (могут работать)
    ("1xbet", "https://1xbet.com"),
    ("melbet", "https://melbet.com"),
    ("betwinner", "https://betwinner.com"),
    ("22bet", "https://22bet.com"),
    ("bet365", "https://bet365.com"),
    ("unibet", "https://unibet.com"),
    ("bwin", "https://bwin.com"),
    ("williamhill", "https://williamhill.com"),
    ("betfair", "https://betfair.com"),
    ("pinnacle", "https://pinnacle.com"),
    ("marathonbet", "https://marathonbet.com"),  # Уже работает
    ("leon", "https://leon.ru"),  # Уже работает
    # Казахстанские/СНГ
    ("fonbet_kz", "https://fonbet.kz"),
    ("parimatch_kz", "https://parimatch.kz"),
    ("1xstavka", "https://1xstavka.ru"),
]

def check_bk(slug, url):
    """Проверяем БК на Cloudflare и доступность API"""
    result = {"slug": slug, "url": url, "status": "unknown"}
    
    try:
        headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        }
        
        # Пробуем загрузить главную
        resp = requests.get(url, headers=headers, timeout=10, allow_redirects=True)
        
        # Проверяем заголовки на Cloudflare
        cf_headers = ["cf-ray", "cf-cache-status", "server"]
        has_cf = any(k in resp.headers for k in cf_headers) and "cloudflare" in resp.headers.get("server", "").lower()
        
        # Проверяем content-type
        ct = resp.headers.get("content-type", "")
        
        # Размер ответа
        size = len(resp.content)
        
        result["http_status"] = resp.status_code
        result["size"] = size
        result["content_type"] = ct[:50]
        result["has_cloudflare"] = has_cf
        
        if has_cf:
            result["status"] = "cloudflare_protected"
        elif resp.status_code == 200 and size > 5000:
            result["status"] = "accessible"
        elif resp.status_code == 200:
            result["status"] = "small_response"
        else:
            result["status"] = f"http_{resp.status_code}"
        
        # Проверяем常见的 API endpoints
        api_endpoints = [
            f"{url}/api/v1/line",
            f"{url}/api/events",
            f"{url}/api/odds",
            f"{url}/api/v1/events",
        ]
        
        working_apis = []
        for api_url in api_endpoints:
            try:
                api_resp = requests.get(api_url, headers={
                    **headers,
                    "Accept": "application/json",
                }, timeout=5)
                if api_resp.status_code == 200:
                    api_ct = api_resp.headers.get("content-type", "")
                    if "json" in api_ct.lower():
                        working_apis.append(api_url)
            except:
                pass
        
        result["working_apis"] = working_apis
        
    except Exception as e:
        result["status"] = f"error: {str(e)[:80]}"
    
    return result

def main():
    print("Проверка БК на Cloudflare и доступность API...")
    print(f"Всего БК для проверки: {len(BK_CANDIDATES)}")
    print()
    
    results = []
    for slug, url in BK_CANDIDATES:
        print(f"Checking {slug}...", end=" ", flush=True)
        result = check_bk(slug, url)
        results.append(result)
        print(f"{result['status']}")
    
    # Сортируем по статусу
    accessible = [r for r in results if r["status"] == "accessible"]
    cloudflare = [r for r in results if "cloudflare" in r["status"]]
    errors = [r for r in results if "error" in r["status"]]
    other = [r for r in results if r not in accessible and r not in cloudflare and r not in errors]
    
    print("\n" + "="*60)
    print("РЕЗУЛЬТАТЫ")
    print("="*60)
    
    print(f"\n✅ ДОСТУПНЫЕ ({len(accessible)}):")
    for r in accessible:
        print(f"  {r['slug']}: {r['url']}")
        if r.get("working_apis"):
            for api in r["working_apis"]:
                print(f"    API: {api}")
    
    print(f"\n⚠️  CLOUDFLARE ({len(cloudflare)}):")
    for r in cloudflare:
        print(f"  {r['slug']}: {r['url']}")
    
    print(f"\n❌ ОШИБКИ ({len(errors)}):")
    for r in errors:
        print(f"  {r['slug']}: {r['status']}")
    
    print(f"\n⏳ ДРУГИЕ ({len(other)}):")
    for r in other:
        print(f"  {r['slug']}: {r['status']}")
    
    # Сохраняем
    with open("bk_check_results.json", "w", encoding="utf-8") as f:
        json.dump({
            "accessible": accessible,
            "cloudflare": cloudflare,
            "errors": errors,
            "other": other,
        }, f, indent=2, ensure_ascii=False)
    
    print(f"\nРезультаты сохранены в bk_check_results.json")

if __name__ == "__main__":
    main()
