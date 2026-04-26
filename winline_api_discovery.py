#!/usr/bin/env python3
"""
Winline - отлавливаем реальные запросы событий
"""

import asyncio
from playwright.async_api import async_playwright
import json

async def test():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True, args=["--disable-blink-features=AutomationControlled"])
        context = await browser.new_context()
        
        page = await context.new_page()
        
        # Перехватываем все ответы
        captured_responses = []
        
        async def on_response(response):
            try:
                url = response.url
                if 'api' in url or 'events' in url.lower() or 'sports' in url.lower():
                    body = await response.text()
                    if len(body) > 100:
                        captured_responses.append({
                            'url': url,
                            'status': response.status,
                            'size': len(body),
                        })
                        
                        if (body.startswith('{') or body.startswith('[')) and len(body) > 500:
                            try:
                                data = json.loads(body)
                                json_str = json.dumps(data)
                                if ('event' in json_str.lower() or 'match' in json_str.lower()) and len(json_str) > 1000:
                                    print(f"\n✅ FOUND LIKELY EVENT DATA:")
                                    print(f"   URL: {url[:100]}")
                                    print(f"   Size: {len(body)} bytes")
                                    if isinstance(data, dict):
                                        print(f"   Keys: {list(data.keys())[:15]}")
                                    elif isinstance(data, list):
                                        print(f"   Array with {len(data)} items")
                                        if data:
                                            print(f"   First item keys: {list(data[0].keys())[:10] if isinstance(data[0], dict) else type(data[0])}")
                            except:
                                pass
            except:
                pass
        
        page.on("response", on_response)
        
        print("Loading Winline main page...")
        await page.goto('https://winline.ru/', timeout=60000, wait_until='domcontentloaded')
        
        print("Waiting 5 seconds for APIs to respond...")
        await asyncio.sleep(5)
        
        print(f"\nTotal responses captured: {len(captured_responses)}")
        print("\nAll API endpoints:")
        for i, resp in enumerate(captured_responses[:20], 1):
            url_short = resp['url'].split('?')[0][-80:]
            print(f"  {i}. {url_short} ({resp['size']} bytes)")
        
        await browser.close()

asyncio.run(test())
