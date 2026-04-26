#!/usr/bin/env python3
"""
Расширенный мониторинг - ждем дольше и ищем события
"""

import asyncio
import json
from playwright.async_api import async_playwright
from datetime import datetime

async def main():
    all_requests = []
    all_responses = []
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=False,  # Видимый браузер чтобы видеть процесс
            args=["--disable-blink-features=AutomationControlled"]
        )
        context = await browser.new_context(
            user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        )
        page = await context.new_page()
        
        async def on_response(response):
            try:
                url = response.url
                status = response.status
                
                content_type = response.headers.get('content-type', '')
                is_json = 'json' in content_type or url.endswith('.json')
                
                body = ""
                try:
                    body = await response.text()
                    if is_json:
                        print(f"\n✅ JSON RESPONSE: {url[:100]}")
                        print(f"   Status: {status}, Size: {len(body)} bytes")
                        # Проверяем есть ли события
                        try:
                            data = json.loads(body)
                            json_str = json.dumps(data)
                            if any(word in json_str.lower() for word in ['event', 'match', 'sport', 'team']):
                                print(f"   🎯 CONTAINS EVENT DATA!")
                                if isinstance(data, dict):
                                    print(f"   Keys: {list(data.keys())[:20]}")
                                elif isinstance(data, list) and data:
                                    print(f"   Array[{len(data)}], first item: {data[0]}")
                        except:
                            pass
                except:
                    pass
                
            except Exception as e:
                pass
        
        page.on("response", on_response)
        
        print("=" * 70)
        print("LOADING WINLINE (headless browser visible)")
        print("=" * 70)
        
        try:
            await page.goto("https://winline.ru/stavki/sport/futbol/", timeout=60000, wait_until="domcontentloaded")
        except Exception as e:
            print(f"⚠️ Navigation error: {e}")
        
        print("\n⏳ WAITING 10 SECONDS FOR EVENTS TO LOAD...")
        for i in range(10):
            await asyncio.sleep(1)
            print(f"  {i+1}s...", end='', flush=True)
        print("\n")
        
        # Попробуем скроллить для lazy-load
        print("📜 Scrolling page for lazy-load events...")
        for _ in range(5):
            await page.evaluate("window.scrollBy(0, 1000)")
            await asyncio.sleep(1)
        
        print("⏳ WAITING 5 MORE SECONDS AFTER SCROLL...")
        await asyncio.sleep(5)
        
        print("\n✅ Complete - check the browser window for what was displayed")
        await browser.close()

asyncio.run(main())
