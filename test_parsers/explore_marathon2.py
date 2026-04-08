import asyncio
from playwright.async_api import async_playwright
import json

results = []

async def explore():
    global results
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True)
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            locale='ru-RU'
        )
        page = await context.new_page()
        
        # Enable console logging
        page.on('console', lambda msg: print(f'Console: {msg.text}'))
        page.on('pageerror', lambda err: print(f'Error: {err}'))
        
        await page.goto('https://www.marathonbet.com/ru/live/football/', wait_until='domcontentloaded', timeout=30000)
        await asyncio.sleep(10)
        
        # Get HTML
        html = await page.content()
        
        # Check if we got actual content or a block page
        result = {
            'html_length': len(html),
            'html_sample': html[0:30000]
        }
        
        results.append(result)
        
        await browser.close()
        
        with open('marathon_results2.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Done - HTML length: ' + str(len(html)))

asyncio.run(explore())
