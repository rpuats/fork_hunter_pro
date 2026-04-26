#!/usr/bin/env python3
"""
Смотрим HTML и ищем события в скрипт тегах или window переменных
"""

import asyncio
import re
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page()
        
        print("Loading page...")
        await page.goto("https://winline.ru/stavki/sport/futbol/", timeout=60000, wait_until="domcontentloaded")
        
        print("Waiting for JS execution...")
        await asyncio.sleep(3)
        
        # Получаем HTML
        print("Getting page content...")
        content = await page.content()
        
        print(f"Page size: {len(content)} bytes")
        
        # Ищем script теги с window переменными
        print("\n🔍 Searching for data in page HTML...\n")
        
        # Ищем window.INITIAL_STATE или похожее
        if 'window.__INITIAL_STATE__' in content:
            print("✅ Found: window.__INITIAL_STATE__")
            match = re.search(r'window\.__INITIAL_STATE__\s*=\s*(\{[^;]+\});', content, re.DOTALL)
            if match:
                data_str = match.group(1)
                print(f"   Size: {len(data_str)} bytes")
                print(f"   Preview: {data_str[:200]}")
        
        # Ищем window.__DATA__
        if 'window.__DATA__' in content:
            print("✅ Found: window.__DATA__")
        
        # Ищем window.REDUX_STATE
        if 'window.REDUX_STATE' in content:
            print("✅ Found: window.REDUX_STATE")
        
        # Ищем window.APP_STATE
        if 'window.APP_STATE' in content:
            print("✅ Found: window.APP_STATE")
        
        # Ищем window.SERVER_DATA
        if 'window.SERVER_DATA' in content:
            print("✅ Found: window.SERVER_DATA")
        
        # Ищем event/match/sport в скрипт тегах
        print("\n🔍 Searching for 'event', 'match', 'sport' in scripts...\n")
        script_tags = re.findall(r'<script[^>]*>([^<]+)</script>', content, re.DOTALL)
        
        for i, script in enumerate(script_tags[:10]):  # Первые 10 скриптов
            if any(word in script.lower() for word in ['event', 'match', 'sport', 'team']):
                print(f"Script {i}: Contains event/match/sport/team")
                print(f"  Size: {len(script)} bytes")
                # Ищем JSON в скрипте
                if '{' in script:
                    # Пытаемся найти JSON объект
                    json_match = re.search(r'(\{[^}]+?"[^"]*(?:event|match|sport|team)[^"]*"[^}]+\})', script)
                    if json_match:
                        print(f"  Found JSON with events!")
                        print(f"  Preview: {json_match.group(1)[:200]}")
        
        # Проверяем window переменные через JS
        print("\n\n🔍 Checking window object via JavaScript...\n")
        
        window_keys = await page.evaluate("""() => {
            const keys = Object.keys(window);
            return keys.filter(k => 
                k.includes('DATA') || 
                k.includes('STATE') || 
                k.includes('event') ||
                k.includes('Event') ||
                k.includes('INITIAL') ||
                k.includes('initial')
            ).slice(0, 20);
        }""")
        
        print(f"Found window keys: {window_keys}\n")
        
        for key in window_keys[:5]:
            try:
                size = await page.evaluate(f"() => {{\n  try {{\n    return JSON.stringify(window.{key}).length;\n  }} catch {{\n    return 'error';\n  }}\n}}")
                print(f"✅ window.{key}: {size} bytes" if isinstance(size, int) else f"⚠️ window.{key}: {size}")
            except:
                pass
        
        await browser.close()

asyncio.run(main())
