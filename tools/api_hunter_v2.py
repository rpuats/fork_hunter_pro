"""API Hunter v2 - находит и тестирует ВСЕ API endpoints БК"""
import asyncio
import json
import sys
import os
import time
import re
import requests
from collections import defaultdict

BK_TARGETS = {
    "winline": "https://winline.ru/football",
    "zenit": "https://zenit.win/line/football",
    "betcity": "https://betcity.ru/ru/line/football",
    "baltbet": "https://baltbet.ru/line",
}

# Паттерны для определения "это события с кэфами"
EVENT_PATTERNS = [
    # Содержит массив событий
    lambda d: isinstance(d, list) and len(d) > 10 and isinstance(d[0], dict),
    # Содержит ключ events/matches/games
    lambda d: isinstance(d, dict) and any(
        k in d and isinstance(d[k], list) and len(d[k]) > 5
        for k in ['events', 'matches', 'games', 'data', 'items', 'lines', 'e', 'm', 't']
    ),
    # Содержит кэфы
    lambda d: _has_odds(d),
]

def _has_odds(data):
    """Проверяет есть ли в данных кэфы"""
    text = json.dumps(data)
    # Ищем паттерны кэфов: 1.50, 2.30, 3.45 и т.д.
    odds = re.findall(r'["\'](?:odds|coef|k|o|odd|coefficient)["\']?\s*:\s*([\d.]+)', text)
    if len(odds) > 10:
        vals = [float(x) for x in odds if x]
        if any(1.01 <= v <= 50 for v in vals):
            return True
    return False

def is_events_data(data):
    """Проверяем это данные о событиях с кэфами"""
    if not data:
        return False
    try:
        return any(p(data) for p in EVENT_PATTERNS)
    except:
        return False

async def intercept_all_responses(page):
    """Перехватываем ВСЕ JSON ответы"""
    responses = []
    
    async def on_response(response):
        try:
            status = response.status
            url = response.url
            ct = response.headers.get('content-type', '')
            
            # Только успешные JSON ответы
            if status == 200 and ('json' in ct or 'javascript' in ct):
                try:
                    body = await response.json()
                    if body:
                        responses.append({
                            'url': url,
                            'content_type': ct,
                            'size': len(json.dumps(body, ensure_ascii=False)),
                            'data': body,
                            'has_events': is_events_data(body),
                            'method': 'GET',  # По умолчанию
                        })
                except:
                    pass
        except:
            pass
    
    page.on('response', on_response)
    return responses

def find_event_api_endpoints(responses):
    """Находим API endpoints с событиями"""
    event_apis = []
    
    for resp in responses:
        if not resp.get('has_events'):
            continue
        
        url = resp['url']
        # Фильтруем аналитику
        if any(skip in url.lower() for skip in [
            'yandex', 'google', 'analytics', 'metric', 'telemetry',
            'cdn.', 'static.', 'assets/', 'icons', 'promo',
            'loyalty', 'bonus', 'settings/desktop', 'alter/1/'
        ]):
            continue
        
        # Проверяем что это API а не страница
        if not any(kw in url.lower() for kw in [
            'api', 'ajax', 'line', 'event', 'odds', 'bet', 'coeff',
            'factor', 'sport', 'match', 'catalog', 'data', 'json',
            'v1', 'v2', 'v3', 'rest', 'graphql', 'bp/', 'xds/',
            'static-data', 'printer', 'get'
        ]):
            continue
        
        data = resp['data']
        
        # Анализируем структуру
        structure = {}
        if isinstance(data, dict):
            structure = {k: len(v) if isinstance(v, list) else type(v).__name__ for k, v in list(data.items())[:10]}
        elif isinstance(data, list):
            structure = f'Array[{len(data)}]'
            if data and isinstance(data[0], dict):
                structure = f'Array[{len(data)}], item_keys: {list(data[0].keys())[:8]}'
        
        event_apis.append({
            'url': url,
            'size': resp['size'],
            'structure': structure,
            'method': resp.get('method', 'GET'),
            'data_sample': json.dumps(data, ensure_ascii=False, default=str)[:500],
        })
    
    return event_apis

async def test_api_endpoint(url, method='GET', headers=None, params=None):
    """Тестируем API endpoint напрямую"""
    try:
        if method == 'POST':
            r = requests.post(url, headers=headers or {}, json=params or {}, timeout=10)
        else:
            r = requests.get(url, headers=headers or {}, params=params or {}, timeout=10)
        
        if r.status_code == 200:
            ct = r.headers.get('content-type', '')
            if 'json' in ct:
                data = r.json()
                if is_events_data(data):
                    return {
                        'success': True,
                        'url': url,
                        'size': len(r.content),
                        'data': data,
                    }
        return {'success': False, 'url': url, 'status': r.status_code}
    except Exception as e:
        return {'success': False, 'url': url, 'error': str(e)}

async def scrape_bk_with_api_hunt(bk_name, start_url):
    """Полный процесс: загрузка + перехват + анализ + тестирование"""
    print(f"\n{'='*60}")
    print(f"  {bk_name.upper()}: {start_url}")
    print(f"{'='*60}")
    
    from playwright.async_api import async_playwright
    
    all_responses = []
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=['--no-sandbox', '--disable-dev-shm-usage']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        
        page = await context.new_page()
        await intercept_all_responses(page)
        
        # 1. Загружаем главную
        print(f"\n  [1/4] Loading main page...")
        try:
            await page.goto(start_url, wait_until='networkidle', timeout=20000)
            await page.wait_for_timeout(3000)
            print(f"    Loaded: {page.url}")
        except Exception as e:
            print(f"    Warning: {str(e)[:80]}")
        
        # 2. Скроллим
        print(f"  [2/4] Scrolling and clicking...")
        try:
            await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 3)")
            await page.wait_for_timeout(1500)
            await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
            await page.wait_for_timeout(1500)
            await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
            await page.wait_for_timeout(1500)
            
            # Кликаем по элементам
            await page.evaluate("""
                () => {
                    const clickable = document.querySelectorAll(
                        'a[href*="sport"], a[href*="live"], [class*="tab"], [class*="sport"], button'
                    );
                    for (let i = 0; i < Math.min(10, clickable.length); i++) {
                        try { clickable[i].click(); } catch(e) {}
                    }
                }
            """)
            await page.wait_for_timeout(3000)
        except Exception as e:
            print(f"    Warning: {str(e)[:80]}")
        
        # 3. Загружаем другие виды спорта
        print(f"  [3/4] Loading other sports...")
        sports_urls = []
        if bk_name == 'winline':
            sports_urls = [
                "https://winline.ru/basketball",
                "https://winline.ru/live/basketball",
                "https://winline.ru/hockey",
                "https://winline.ru/live/hockey",
            ]
        elif bk_name == 'zenit':
            sports_urls = [
                "https://zenit.win/line/basketball",
                "https://zenit.win/live/basketball",
                "https://zenit.win/line/hockey",
            ]
        elif bk_name == 'betcity':
            sports_urls = [
                "https://betcity.ru/ru/line/basketball",
                "https://betcity.ru/ru/live/basketball",
            ]
        elif bk_name == 'baltbet':
            sports_urls = [
                "https://baltbet.ru/line/basketball",
                "https://baltbet.ru/live/basketball",
            ]
        
        for sport_url in sports_urls:
            try:
                await page.goto(sport_url, wait_until='domcontentloaded', timeout=15000)
                await page.wait_for_timeout(2000)
                await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                await page.wait_for_timeout(1500)
            except:
                pass
        
        # 4. Анализируем
        print(f"  [4/4] Analyzing {len(all_responses)} responses...")
        event_apis = find_event_api_endpoints(all_responses)
        
        await browser.close()
    
    return event_apis

async def main():
    bk_name = sys.argv[1] if len(sys.argv) > 1 else None
    output_dir = os.path.dirname(os.path.abspath(__file__))
    
    if bk_name:
        if bk_name not in BK_TARGETS:
            print(f"Unknown BK: {bk_name}")
            return
        apis = await scrape_bk_with_api_hunt(bk_name, BK_TARGETS[bk_name])
        
        print(f"\n{'='*60}")
        print(f"  FOUND {len(apis)} POTENTIAL EVENT APIs")
        print(f"{'='*60}")
        
        for i, api in enumerate(apis):
            print(f"\n  [{i+1}] {api['url'][:100]}")
            print(f"      Size: {api['size']:,} bytes")
            print(f"      Structure: {api['structure']}")
            print(f"      Sample: {api['data_sample'][:200]}")
        
        # Сохраняем
        output_file = os.path.join(output_dir, '..', f'{bk_name}_api_results.json')
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump({
                'bk': bk_name,
                'apis': apis,
                'count': len(apis),
                'timestamp': time.time()
            }, f, ensure_ascii=False, indent=2)
        print(f"\n  Saved to {output_file}")
        
    else:
        # Сканируем все БК
        for bk, url in BK_TARGETS.items():
            apis = await scrape_bk_with_api_hunt(bk, url)
            print(f"  {bk}: {len(apis)} potential APIs found")
            await asyncio.sleep(2)

if __name__ == "__main__":
    asyncio.run(main())
