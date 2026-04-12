"""Находим реальный API с событиями Winline"""
import asyncio
import json
from playwright.async_api import async_playwright
from playwright_stealth import Stealth

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True, args=['--no-sandbox'])
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
        )
        Stealth().apply_stealth_sync(context)
        page = await context.new_page()
        
        # Перехватываем ВСЕ responses
        async def on_response(response):
            url = response.url
            if response.status == 200:
                # Ищем большие JSON ответы (вероятно события)
                ct = response.headers.get('content-type', '')
                if 'json' in ct:
                    try:
                        body = await response.json()
                        size = len(json.dumps(body))
                        if size > 5000:  # Большие ответы
                            keys = list(body.keys()) if isinstance(body, dict) else f'Array[{len(body)}]'
                            print(f"  URL: {url[:100]}")
                            print(f"  Size: {size:,} bytes, Keys: {keys}")
                            # Ищем признаки событий
                            if isinstance(body, dict):
                                for k in body:
                                    v = body[k]
                                    if isinstance(v, list) and len(v) > 5:
                                        print(f"    {k}: Array[{len(v)}]")
                                        if isinstance(v[0], dict):
                                            print(f"      Keys: {list(v[0].keys())[:8]}")
                            elif isinstance(body, list) and len(body) > 5:
                                if isinstance(body[0], dict):
                                    print(f"    Item keys: {list(body[0].keys())[:8]}")
                            print()
                    except:
                        pass
        
        page.on('response', on_response)
        
        # Загружаем и кликаем по разным видам спорта
        await page.goto('https://winline.ru/football', wait_until='networkidle', timeout=30000)
        await page.wait_for_timeout(5000)
        
        # Скроллим и ждём
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await page.wait_for_timeout(3000)
        
        # Пробуем другие виды спорта
        for sport_url in [
            'https://winline.ru/basketball',
            'https://winline.ru/hockey',
            'https://winline.ru/tennis',
        ]:
            try:
                print(f"Loading {sport_url}...")
                await page.goto(sport_url, wait_until='networkidle', timeout=20000)
                await page.wait_for_timeout(3000)
                await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                await page.wait_for_timeout(2000)
            except:
                pass
        
        await browser.close()

asyncio.run(main())
