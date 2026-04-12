"""Тест Winline через Playwright с перехватом network запросов"""
import asyncio
import json
from playwright.async_api import async_playwright

async def main():
    print("🔍 Запускаем Winline с перехватом API...")
    
    pw = await async_playwright().start()
    browser = await pw.chromium.launch(headless=True)
    context = await browser.new_context()
    page = await context.new_page()
    
    # Перехватываем все XHR/Fetch запросы
    api_calls = []
    
    async def handle_request(request):
        if request.resource_type in ['xhr', 'fetch']:
            api_calls.append({
                'url': request.url,
                'method': request.method,
                'headers': dict(request.headers),
            })
    
    page.on('request', handle_request)
    
    # Загружаем страницу
    print("📄 Загружаем winline.ru/football...")
    await page.goto('https://winline.ru/football', wait_until='domcontentloaded', timeout=30000)
    await asyncio.sleep(5)
    
    print(f"\n📊 Перехвачено {len(api_calls)} API запросов:")
    for call in api_calls:
        print(f"\n  {call['method']} {call['url']}")
    
    # Сохраняем в файл
    with open('winline_api_calls.json', 'w') as f:
        json.dump(api_calls, f, indent=2, ensure_ascii=False)
    
    print("\n✅ Сохранено в winline_api_calls.json")
    
    await browser.close()

if __name__ == '__main__':
    asyncio.run(main())
