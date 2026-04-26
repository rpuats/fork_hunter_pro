#!/usr/bin/env python3
"""
Перехватываем ВСЕ запросы включая XHR/fetch
"""

import asyncio
import json
from playwright.async_api import async_playwright

async def main():
    all_apis = {}
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context()
        page = await context.new_page()
        
        async def on_request(request):
            url = request.url
            if '/api/' in url or '/xds/' in url or '/cyber/' in url:
                # Вытаскиваем базовый URL без параметров
                base_url = url.split('?')[0]
                if base_url not in all_apis:
                    all_apis[base_url] = {'method': request.method, 'count': 0}
                all_apis[base_url]['count'] += 1
        
        page.on("request", on_request)
        
        print("=" * 70)
        print("LOADING WINLINE AND NAVIGATING")
        print("=" * 70)
        
        try:
            print("\n1. Loading main page...")
            await page.goto("https://winline.ru/", timeout=60000, wait_until="domcontentloaded")
            print(f"   Found {len(all_apis)} API endpoints so far")
            
            await asyncio.sleep(2)
            
            print("\n2. Navigating to /live...")
            await page.goto("https://winline.ru/live", timeout=60000, wait_until="domcontentloaded")
            print(f"   Found {len(all_apis)} API endpoints so far")
            
            await asyncio.sleep(2)
            
            print("\n3. Navigating to /stavki/sport/futbol...")
            await page.goto("https://winline.ru/stavki/sport/futbol/", timeout=60000, wait_until="domcontentloaded")
            print(f"   Found {len(all_apis)} API endpoints so far")
            
            await asyncio.sleep(2)
            
        except Exception as e:
            print(f"Navigation error: {e}")
        
        print("\n" + "=" * 70)
        print("ALL API ENDPOINTS CALLED:")
        print("=" * 70)
        
        for url in sorted(all_apis.keys()):
            info = all_apis[url]
            print(f"\n{info['method']} {url}")
            print(f"  Called {info['count']} times")
        
        # Сохраняем все
        with open('winline_all_apis.json', 'w', encoding='utf-8') as f:
            json.dump(all_apis, f, indent=2, ensure_ascii=False)
        
        print(f"\n💾 Saved {len(all_apis)} endpoints to winline_all_apis.json")
        
        await browser.close()

asyncio.run(main())
