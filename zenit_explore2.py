import asyncio
from playwright.async_api import async_playwright
import json
import sys
import io

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')

async def explore():
    results = []
    results.append('=== Zenit Bookmaker Exploration ===')
    
    pw = await async_playwright().start()
    
    try:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled'])
        page = await browser.new_page()
        
        await page.set_viewport_size({'width': 1920, 'height': 1080})
        
        results.append('1. Navigating to https://zenit.win/live...')
        await page.goto('https://zenit.win/live', wait_until='domcontentloaded', timeout=30000)
        
        results.append('2. Waiting 10 seconds...')
        await asyncio.sleep(10)
        
        title = await page.title()
        results.append(f'   Page title: {title}')
        
        current_url = page.url
        results.append(f'   Current URL: {current_url}')
        
        page_text = await page.locator('body').inner_text()
        results.append(f'   Page text length: {len(page_text)} chars')
        results.append(f'   First 500 chars: {page_text[:500]}')
        
        results.append('')
        results.append('3. Searching for event containers...')
        event_selectors = ['.event', '.sport-event', '.live-event', '.match', '.bb-Nm', '[class*=\
event\]']
        for sel in event_selectors:
            try:
                count = await page.locator(sel).count()
                if count > 0:
                    results.append(f'   Found {count} with: {sel}')
            except:
                pass
        
        results.append('')
        results.append('4. Searching for odds elements...')
        odds_selectors = ['.odd', '.odds', '.coefficient', '.rate']
        for sel in odds_selectors:
            try:
                count = await page.locator(sel).count()
                if count > 0:
                    results.append(f'   Found {count} with: {sel}')
            except:
                pass
        
        results.append('')
        results.append('5. Getting sample HTML...')
        body_html = await page.locator('body').inner_html()
        results.append(body_html[:10000])
        
        await browser.close()
    except Exception as e:
        results.append(f'Error: {e}')
        try:
            await browser.close()
        except:
            pass
    
    await pw.stop()
    
    with open('zenit_results.txt', 'w', encoding='utf-8') as f:
        f.write('\\n'.join(results))
    print('Results written to zenit_results.txt')

asyncio.run(explore())
