import asyncio
from playwright.async_api import async_playwright
import json

async def explore():
    print('=== Zenit Bookmaker Exploration ===')
    print()
    
    pw = await async_playwright().start()
    
    try:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled'])
        page = await browser.new_page()
        
        await page.set_viewport_size({'width': 1920, 'height': 1080})
        
        print('1. Navigating to https://zenit.win/live...')
        await page.goto('https://zenit.win/live', wait_until='domcontentloaded', timeout=30000)
        
        print('2. Waiting 10 seconds...')
        await asyncio.sleep(10)
        
        title = await page.title()
        print(f'   Page title: {title}')
        
        current_url = page.url
        print(f'   Current URL: {current_url}')
        
        page_text = await page.locator('body').inner_text()
        print(f'   Page text length: {len(page_text)} chars')
        print(f'   First 500 chars: {page_text[:500]}')
        
        print()
        print('3. Searching for event containers...')
        event_selectors = ['.event', '.sport-event', '.live-event', '.match', '.bb-Nm', '[class*=\
event\]']
        for sel in event_selectors:
            try:
                count = await page.locator(sel).count()
                if count > 0:
                    print(f'   Found {count} with: {sel}')
            except:
                pass
        
        print()
        print('4. Searching for odds elements...')
        odds_selectors = ['.odd', '.odds', '.coefficient', '.rate']
        for sel in odds_selectors:
            try:
                count = await page.locator(sel).count()
                if count > 0:
                    print(f'   Found {count} with: {sel}')
            except:
                pass
        
        await browser.close()
    except Exception as e:
        print(f'Error: {e}')
        try:
            await browser.close()
        except:
            pass
    
    await pw.stop()

asyncio.run(explore())
