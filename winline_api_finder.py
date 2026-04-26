#!/usr/bin/env python3
"""
Ловим реальный эндпоинт для событий - кликаем на спорт и смотрим запросы
"""

import asyncio
import json
from playwright.async_api import async_playwright

async def main():
    event_requests = []
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context()
        page = await context.new_page()
        
        async def on_response(response):
            url = response.url
            # Ищем только JSON запросы к API
            if ('/api/' in url or '/xds/' in url) and 'json' in response.headers.get('content-type', ''):
                try:
                    body = await response.text()
                    if len(body) > 500:  # Только большие ответы
                        event_requests.append({
                            'url': url,
                            'size': len(body),
                            'status': response.status
                        })
                        # Ищем слова указывающие на события
                        if any(w in body.lower() for w in ['event', 'match', 'sport', 'team', 'live']):
                            print(f"\n✅ {url}")
                except:
                    pass
        
        page.on("response", on_response)
        
        print("Loading Winline...")
        await page.goto("https://winline.ru/", timeout=60000, wait_until="domcontentloaded")
        
        await asyncio.sleep(2)
        
        print("\nClicking on Football...")
        try:
            # Пытаемся найти и кликнуть на футбол
            await page.click("text=Футбол", timeout=5000)
            await asyncio.sleep(3)
        except:
            print("Could not click Football, trying another selector...")
            try:
                await page.click("a:has-text('Футбол')", timeout=5000)
                await asyncio.sleep(3)
            except:
                print("Could not find Football button")
        
        print("\nWaiting for event data...")
        await asyncio.sleep(3)
        
        print("\nAll captured API endpoints:")
        for req in event_requests:
            print(f"  {req['url'][:100]} ({req['size']} bytes)")
        
        # Сохраняем для анализа
        with open('winline_api_endpoints.json', 'w') as f:
            json.dump(event_requests, f, indent=2)
        
        await browser.close()

asyncio.run(main())
