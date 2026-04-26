#!/usr/bin/env python3
"""
Просто открываем браузер и смотрим что там на экране
"""

import asyncio
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=False)  # ВИДИМЫЙ браузер!
        page = await browser.new_page(viewport={"width": 1440, "height": 900})
        
        print("=" * 70)
        print("OPENING WINLINE IN VISIBLE BROWSER")
        print("=" * 70)
        print("\nBrowser window should open now.")
        print("Look at what's displayed - do you see events/matches?")
        print("\nWill keep the browser open for 30 seconds...")
        print()
        
        try:
            await page.goto("https://winline.ru/stavki/sport/futbol/", timeout=60000, wait_until="domcontentloaded")
        except Exception as e:
            print(f"Error: {e}")
        
        # Ждем 30 секунд чтобы можно было посмотреть
        for i in range(30):
            await asyncio.sleep(1)
            print(f"  {i+1}s...", end='', flush=True)
        
        print("\n\nClosing browser...")
        await browser.close()

asyncio.run(main())
