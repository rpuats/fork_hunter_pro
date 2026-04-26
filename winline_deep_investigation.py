"""
Парсер Winline - ищет события в загруженной странице и JavaScript
"""
import asyncio
import json
import re
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()
        
        # Перехватываем все запросы/ответы
        all_data = {
            'requests': [],
            'responses': [],
            'events_found': []
        }
        
        def handle_response(response):
            url = response.url
            # Ищем события в ответах
            if any(x in url for x in ['api', 'events', 'feed', 'line', 'sport']):
                all_data['requests'].append(url)
        
        page.on('response', handle_response)
        
        print("🔄 Loading winline.ru...")
        try:
            await page.goto('https://winline.ru/stavki/sport/futbol/', wait_until='load', timeout=60000)
        except Exception as e:
            print(f"⚠️  Navigation warning: {e}")
        
        print("⏳ Waiting for page to hydrate (10 seconds)...")
        await asyncio.sleep(10)
        
        # Получаем полный HTML
        html = await page.content()
        print(f"📄 HTML size: {len(html)} bytes")
        
        # Ищем JSON в HTML
        json_matches = re.findall(r'window\.__[A-Z_]+\s*=\s*(\{[^}]+\}|"[^"]*")', html)
        print(f"🔍 Found {len(json_matches)} window variables")
        
        # Ищем события в HTML напрямую
        if 'event' in html.lower():
            print("✓ Word 'event' found in HTML")
            # Ищем паттерны ID событий (обычно числа)
            event_ids = re.findall(r'"eventId"\s*:\s*(\d+)', html, re.IGNORECASE)
            print(f"  Found {len(set(event_ids))} unique event IDs")
        
        # Проверяем script теги
        scripts = re.findall(r'<script[^>]*>(.+?)</script>', html, re.DOTALL)
        print(f"🔍 Found {len(scripts)} script tags")
        
        total_script_size = sum(len(s) for s in scripts)
        print(f"  Total script content: {total_script_size} bytes")
        
        # Пытаемся выполнить JavaScript и получить события
        print("\n🔧 Trying to extract events via JavaScript...")
        
        # Метод 1: Проверяем window переменные
        try:
            result = await page.evaluate('''
            () => {
                const findings = {
                    window_keys: Object.keys(window).filter(k => k.includes('event') || k.includes('Event') || k.includes('sport')).slice(0, 20),
                    has_react: typeof window.__REACT_DEVTOOLS_GLOBAL_HOOK__ !== 'undefined',
                    has_redux: typeof window.__REDUX_DEVTOOLS_EXTENSION__ !== 'undefined',
                };
                
                // Ищем data в DOM
                const scripts = document.querySelectorAll('script[type="application/json"]');
                const json_scripts = [];
                for (let s of scripts) {
                    if (s.textContent && s.textContent.length > 100) {
                        json_scripts.push({
                            size: s.textContent.length,
                            preview: s.textContent.slice(0, 200)
                        });
                    }
                }
                findings.json_scripts = json_scripts;
                
                return findings;
            }
            ''')
            print(f"  Window keys with 'event'/'sport': {result['window_keys']}")
            print(f"  Has React DevTools: {result['has_react']}")
            print(f"  Has Redux: {result['has_redux']}")
            print(f"  JSON script tags: {len(result['json_scripts'])}")
            for i, script in enumerate(result['json_scripts'][:3]):
                print(f"    Script {i}: {script['size']} bytes")
        except Exception as e:
            print(f"  ⚠️  JavaScript execution error: {e}")
        
        # Метод 2: Ищем iframe (Web Components часто используют их)
        try:
            iframes = await page.query_selector_all('iframe')
            print(f"\n📦 Found {len(iframes)} iframes")
            
            if iframes:
                for i, iframe in enumerate(iframes[:3]):
                    try:
                        src = await iframe.get_attribute('src')
                        print(f"  iframe {i}: src={src}")
                    except:
                        pass
        except Exception as e:
            print(f"  ⚠️  Iframe check error: {e}")
        
        # Метод 3: Смотрим Network tab через CDP
        print("\n📡 Checking all network requests...")
        
        # Перезагружаемся и ловим запросы
        browser2 = await p.chromium.launch(headless=True)
        context = await browser2.new_context()
        page2 = await context.new_page()
        
        captured_responses = []
        
        async def capture_response(response):
            url = response.url
            if any(x in url for x in ['api', 'graphql', 'feed', 'events', 'sport', 'line']):
                try:
                    text = await response.text()
                    captured_responses.append({
                        'url': url,
                        'status': response.status,
                        'size': len(text),
                        'type': response.headers.get('content-type', ''),
                        'preview': text[:300] if len(text) > 0 else ''
                    })
                    if response.status == 200 and len(text) > 100:
                        print(f"  ✓ {response.status} {url[:80]}")
                except:
                    pass
        
        page2.on('response', capture_response)
        
        try:
            await page2.goto('https://winline.ru/stavki/sport/futbol/', wait_until='load', timeout=60000)
            await asyncio.sleep(5)
        except:
            pass
        
        print(f"\n📊 Captured {len(captured_responses)} API responses")
        for resp in captured_responses:
            print(f"\n  URL: {resp['url']}")
            print(f"  Status: {resp['status']}, Type: {resp['type']}, Size: {resp['size']}")
            if resp['size'] > 100:
                # Пытаемся спарсить как JSON
                try:
                    data = json.loads(resp['preview'])
                    if isinstance(data, dict):
                        print(f"  Keys: {list(data.keys())[:15]}")
                    elif isinstance(data, list):
                        print(f"  Array with {resp['size']} items")
                except:
                    # Может быть текст
                    if 'html' in resp['type'].lower():
                        print(f"  HTML content (first 150 chars): {resp['preview'][:150]}")
                    else:
                        print(f"  Content: {resp['preview'][:150]}")
        
        await browser.close()
        await browser2.close()

if __name__ == '__main__':
    asyncio.run(main())
