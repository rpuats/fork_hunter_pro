"""
Запусти РЕАЛЬНЫЙ браузер (видимый) и смотри что грузится
"""
import asyncio
from playwright.async_api import async_playwright
import json
import sys

sys.stdout.reconfigure(encoding='utf-8')

async def main():
    async with async_playwright() as p:
        # РЕАЛЬНЫЙ браузер - видимый!
        browser = await p.chromium.launch(headless=False)  # headless=False = видимый браузер!
        context = await browser.new_context()
        
        # Собираем все запросы
        requests_data = []
        responses_data = []
        
        async def handle_request(request):
            url = request.url
            if 'api' in url or 'events' in url.lower() or 'xds' in url or 'cls' in url:
                requests_data.append({
                    'url': url,
                    'method': request.method,
                    'headers': dict(request.headers)
                })
                print(f"📤 REQUEST: {url[:100]}")
        
        async def handle_response(response):
            url = response.url
            try:
                if response.status == 200:
                    text = await response.text()
                    if len(text) > 50:
                        responses_data.append({
                            'url': url,
                            'status': response.status,
                            'content_type': response.headers.get('content-type', 'unknown'),
                            'size': len(text),
                            'content_preview': text[:500]
                        })
                        print(f"📥 RESPONSE {response.status}: {url[:80]} ({len(text)} bytes, {response.headers.get('content-type')})")
            except:
                pass
        
        page = await context.new_page()
        page.on('request', handle_request)
        page.on('response', handle_response)
        
        print("🔄 Loading https://winline.ru...")
        await page.goto('https://winline.ru/stavki/sport/futbol/')
        
        print("\n⏳ Waiting 10 seconds for events to load...")
        await asyncio.sleep(10)
        
        print("\n📊 Scrolling to trigger lazy-loading...")
        for i in range(5):
            await page.evaluate('window.scrollBy(0, 500)')
            await asyncio.sleep(2)
        
        print("\n🔍 All API/XDS/CLS requests found:")
        for req in requests_data:
            print(f"  ➡️  {req['url']}")
        
        print("\n🔍 All significant responses:")
        for resp in responses_data:
            if len(resp['content_preview']) > 100:
                print(f"  ⬅️  {resp['url'][:70]}")
                print(f"      Status: {resp['status']}, Type: {resp['content_type']}, Size: {resp['size']}")
                
                # Если это JSON или похоже на события - покажи первые ключи
                try:
                    if 'json' in resp['content_type']:
                        data = json.loads(resp['content_preview'])
                        if isinstance(data, dict):
                            print(f"      Keys: {list(data.keys())[:10]}")
                        elif isinstance(data, list):
                            print(f"      Array with {resp['size']} items")
                except:
                    pass
        
        print("\n✅ Check the browser window - it should still be open!")
        print("📍 In browser DevTools, check Network tab for event-related requests")
        
        # Закрой браузер когда пользователь нажмет Enter
        input("\n⏸️  Press Enter to close browser...")
        
        await browser.close()

if __name__ == '__main__':
    asyncio.run(main())
