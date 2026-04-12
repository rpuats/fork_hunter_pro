"""Перехват реальных API запросов БК через Playwright network interception"""
import asyncio
import json
import os
from playwright.async_api import async_playwright

RESULTS_FILE = "intercepted_apis.json"
results = {}

async def intercept_bk(name, urls, output_key):
    """Загружаем страницы и перехватываем все XHR/Fetch запросы"""
    print(f"\n🎯 {name}: перехват API запросов...")
    results[output_key] = []
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=['--disable-blink-features=AutomationControlled', '--no-sandbox']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
        )
        
        # Перехватываем ВСЕ запросы
        intercepted = []
        
        async def handle_request(request):
            if request.resource_type in ['xhr', 'fetch', 'websocket']:
                intercepted.append({
                    'url': request.url,
                    'method': request.method,
                    'type': request.resource_type,
                    'headers': dict(request.headers),
                })
        
        page = await context.new_page()
        page.on('request', handle_request)
        
        for url in urls:
            print(f"  📄 Загружаем {url}")
            try:
                await page.goto(url, wait_until='networkidle', timeout=30000)
                await asyncio.sleep(3)  # Ждём дополнительные запросы
            except Exception as e:
                print(f"  ⚠️  Ошибка загрузки: {e}")
        
        print(f"  ✅ Перехвачено {len(intercepted)} запросов")
        
        # Фильтруем только API-подобные URL
        api_requests = []
        for req in intercepted:
            url = req['url']
            if any(kw in url.lower() for kw in ['api', 'line', 'event', 'odds', 'bet', 'factor', 'coeff', 'sport']):
                api_requests.append(req)
                print(f"  🔍 API: {req['method']} {url[:100]}")
        
        results[output_key] = api_requests
        
        await browser.close()

async def main():
    print("=" * 80)
    print("🔍 ПЕРЕХВАТ API ЗАПРОСОВ БК ЧЕРЕЗ PLAYWRIGHT")
    print("=" * 80)
    
    # Winline
    await intercept_bk(
        "Winline",
        ["https://winline.ru/football"],
        "winline"
    )
    
    # Zenit
    await intercept_bk(
        "Zenit", 
        ["https://zenit.win/line/football"],
        "zenit"
    )
    
    # Betcity
    await intercept_bk(
        "Betcity",
        ["https://betcity.ru/ru/line/football"],
        "betcity"
    )
    
    # Baltbet
    await intercept_bk(
        "Baltbet",
        ["https://baltbet.ru/line"],
        "baltbet"
    )
    
    # Сохраняем результаты
    with open(RESULTS_FILE, 'w', encoding='utf-8') as f:
        json.dump(results, f, indent=2, ensure_ascii=False)
    
    print(f"\n📊 Результаты сохранены в {RESULTS_FILE}")
    print(f"\n📈 Итого перехвачено API запросов:")
    for bk, apis in results.items():
        print(f"  {bk}: {len(apis)} запросов")

if __name__ == "__main__":
    asyncio.run(main())
