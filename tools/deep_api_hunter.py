"""Глубокий охотник за API - загружает БК через Playwright и перехватывает ВСЕ запросы"""
import asyncio
import json
import sys
import os
import time
from collections import defaultdict

BK_TARGETS = {
    "winline": "https://winline.ru/football",
    "zenit": "https://zenit.win/line/football",
    "betcity": "https://betcity.ru/ru/line/football",
    "baltbet": "https://baltbet.ru/line",
}

OUTPUT_DIR = os.path.dirname(os.path.abspath(__file__))

async def intercept_all_requests(page, bk_name):
    """Перехватываем ВСЕ запросы со страницы"""
    all_requests = []
    all_responses = []
    
    async def on_request(request):
        if request.resource_type in ['xhr', 'fetch', 'websocket', 'script', 'document']:
            all_requests.append({
                'url': request.url,
                'method': request.method,
                'type': request.resource_type,
                'headers': dict(request.headers),
            })
    
    async def on_response(response):
        if response.status == 200:
            url = response.url
            # Фильтруем только интересные URL
            interesting = any(kw in url.lower() for kw in [
                'api', 'ajax', 'line', 'event', 'odds', 'bet', 'coeff', 
                'factor', 'sport', 'match', 'catalog', 'data', 'json',
                'v1', 'v2', 'v3', 'rest', 'graphql'
            ])
            
            if interesting:
                try:
                    content_type = response.headers.get('content-type', '')
                    if 'json' in content_type or 'javascript' in content_type:
                        body = await response.json()
                        all_responses.append({
                            'url': url,
                            'content_type': content_type,
                            'size': len(json.dumps(body, ensure_ascii=False)),
                            'keys': list(body.keys()) if isinstance(body, dict) else f'Array[{len(body)}]',
                            'preview': json.dumps(body, ensure_ascii=False, default=str)[:500],
                        })
                    elif 'html' in content_type and len(url) > 50:
                        # Может быть API endpoint возвращающий HTML
                        body = await response.text()
                        if len(body) > 100:
                            all_responses.append({
                                'url': url,
                                'content_type': content_type,
                                'size': len(body),
                                'keys': 'HTML response',
                                'preview': body[:300],
                            })
                except:
                    pass
    
    page.on('request', on_request)
    page.on('response', on_response)
    return all_requests, all_responses

async def scrape_bk_deep(bk_name, url):
    """Глубокий скрапинг одной БК"""
    print(f"\n{'='*60}")
    print(f"  {bk_name.upper()}: {url}")
    print(f"{'='*60}")
    
    from playwright.async_api import async_playwright
    from playwright_stealth import Stealth
    
    all_found_apis = []
    
    async with async_playwright() as p:
        # Пробуем разные конфигурации браузера
        configs = [
            # Desktop Chrome
            {
                'user_agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
                'viewport': {'width': 1920, 'height': 1080},
            },
            # Mobile Safari
            {
                'user_agent': 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
                'viewport': {'width': 390, 'height': 844},
            },
        ]
        
        for cfg_idx, cfg in enumerate(configs):
            print(f"\n  📱 Конфигурация {cfg_idx+1}: {cfg['user_agent'][:50]}...")
            
            browser = await p.chromium.launch(
                headless=True,
                args=[
                    '--no-sandbox',
                    '--disable-blink-features=AutomationControlled',
                    '--disable-dev-shm-usage',
                    '--disable-web-security',
                    '--disable-features=IsolateOrigins,site-per-process',
                    '--disable-infobars',
                ]
            )
            
            context = await browser.new_context(
                user_agent=cfg['user_agent'],
                viewport=cfg['viewport'],
                locale='ru-RU',
                timezone_id='Europe/Moscow',
            )
            
            # Применяем stealth
            Stealth().apply_stealth_sync(context)
            
            # Инициализируем перехватчик
            page = await context.new_page()
            all_requests, all_responses = await intercept_all_requests(page, bk_name)
            
            try:
                # Загружаем страницу с разными стратегиями
                print(f"    Загрузка страницы...")
                
                # Пробуем разные wait_until
                for wait_mode in ['domcontentloaded', 'networkidle']:
                    try:
                        await page.goto(url, wait_until=wait_mode, timeout=20000)
                        print(f"    ✅ {wait_mode} загружен")
                        
                        # Ждём дополнительно для загрузки API
                        await page.wait_for_timeout(5000)
                        
                        # Скроллим для lazy loading
                        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 3)")
                        await page.wait_for_timeout(2000)
                        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
                        await page.wait_for_timeout(2000)
                        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                        await page.wait_for_timeout(2000)
                        
                        # Кликаем по элементам для триггера API
                        await page.evaluate("""
                            () => {
                                // Кликаем по sport tabs
                                document.querySelectorAll('[class*="sport"], [class*="tab"], a[href*="sport"]').forEach(el => {
                                    try { el.click(); } catch(e) {}
                                });
                            }
                        """)
                        await page.wait_for_timeout(3000)
                        
                    except Exception as e:
                        print(f"    ⚠️  {wait_mode} error: {str(e)[:100]}")
                        continue
                
                # Анализируем результаты
                print(f"\n    📊 Перехвачено {len(all_requests)} запросов, {len(all_responses)} ответов")
                
                # Ищем API endpoints
                api_endpoints = []
                for resp in all_responses:
                    url = resp['url']
                    # Фильтруем analytics и CDN
                    if any(skip in url for skip in ['yandex.ru', 'google', 'analytics', 'cdn.', 'static.']):
                        continue
                    
                    # Проверяем есть ли данные
                    if resp.get('size', 0) > 500:
                        api_endpoints.append(resp)
                        print(f"    🔍 API: {url[:80]}")
                        print(f"       Size: {resp['size']:,} bytes, Keys: {resp['keys']}")
                        if 'preview' in resp:
                            print(f"       Preview: {resp['preview'][:200]}")
                        print()
                
                all_found_apis.extend(api_endpoints)
                
                # Сохраняем сырые данные для анализа
                if api_endpoints:
                    output_file = os.path.join(OUTPUT_DIR, f"../{bk_name}_api_config{cfg_idx+1}.json")
                    with open(output_file, 'w', encoding='utf-8') as f:
                        json.dump({
                            'bk': bk_name,
                            'config': cfg_idx + 1,
                            'apis': api_endpoints,
                            'total_requests': len(all_requests),
                            'timestamp': time.time()
                        }, f, ensure_ascii=False, indent=2)
                    print(f"    💾 Сохранено: {output_file}")
                
            except Exception as e:
                print(f"    ❌ Ошибка: {e}")
            
            await browser.close()
    
    return all_found_apis

async def main():
    bk_name = sys.argv[1] if len(sys.argv) > 1 else None
    
    if bk_name:
        if bk_name in BK_TARGETS:
            apis = await scrape_bk_deep(bk_name, BK_TARGETS[bk_name])
            print(f"\n✅ {bk_name}: найдено {len(apis)} API endpoint'ов")
        else:
            print(f"❌ Unknown BK: {bk_name}")
            print(f"Available: {list(BK_TARGETS.keys())}")
    else:
        # Сканируем все БК
        for bk, url in BK_TARGETS.items():
            apis = await scrape_bk_deep(bk, url)
            print(f"\n✅ {bk}: найдено {len(apis)} API endpoint'ов")
            await asyncio.sleep(2)  # Пауза между БК
        
        # Сводный отчёт
        print(f"\n{'='*60}")
        print("📊 ИТОГОВЫЙ ОТЧЁТ")
        print(f"{'='*60}")
        for bk in BK_TARGETS:
            files = [f for f in os.listdir(os.path.join(OUTPUT_DIR, '..')) if f.startswith(f'{bk}_api_')]
            print(f"  {bk}: {len(files)} файлов с API данными")

if __name__ == "__main__":
    asyncio.run(main())
