"""
Ловим все сетевые запросы и анализируем ответы на предмет событий
"""
import asyncio
import json
from playwright.async_api import async_playwright
from datetime import datetime

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True, args=['--no-sandbox'])
        context = await browser.new_context()
        page = await context.new_page()
        
        all_requests = []
        all_responses = []
        
        async def handle_response(response):
            url = response.url
            try:
                if response.status == 200:
                    text = await response.text()
                    
                    # Проверяем на события
                    if any(x in url.lower() for x in ['api', 'graphql', 'event', 'feed', 'line', 'sport', 'match']):
                        all_responses.append({
                            'url': url[:120],
                            'status': response.status,
                            'type': response.headers.get('content-type', ''),
                            'size': len(text),
                            'has_events': 'event' in text.lower() or 'match' in text.lower() or '1x2' in text.lower(),
                            'preview': text[:300]
                        })
                        print(f"[{response.status}] {url[:100]}")
                        print(f"  Type: {response.headers.get('content-type')}, Size: {len(text)}")
                        
                        if len(text) > 100:
                            # Пытаемся как JSON
                            try:
                                data = json.loads(text[:5000])
                                if isinstance(data, dict):
                                    print(f"  JSON keys: {list(data.keys())[:10]}")
                                elif isinstance(data, list):
                                    print(f"  JSON array with {len(data)} items")
                            except:
                                if 'event' in text[:500].lower():
                                    print(f"  Contains 'event'!")
            except:
                pass
        
        page.on('response', handle_response)
        
        url = "https://winline.ru/stavki/sport/futbol/"
        print(f"Loading {url}...")
        print("Capturing all network responses...\n")
        
        try:
            await page.goto(url, wait_until='load', timeout=60000)
        except:
            pass
        
        await asyncio.sleep(5)
        print("\n" + "="*80)
        print("SUMMARY")
        print("="*80)
        print(f"Responses captured: {len(all_responses)}")
        
        # Analyze
        json_responses = [r for r in all_responses if 'json' in r['type'].lower()]
        print(f"JSON responses: {len(json_responses)}")
        
        for resp in json_responses:
            print(f"\n  URL: {resp['url']}")
            print(f"  Size: {resp['size']}")
            print(f"  Has events keyword: {resp['has_events']}")
            if resp['size'] < 5000:
                print(f"  Content: {resp['preview'][:500]}")
        
        await browser.close()

asyncio.run(main())
