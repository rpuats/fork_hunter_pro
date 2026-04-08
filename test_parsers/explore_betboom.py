import asyncio
from playwright.async_api import async_playwright
import json

results = []

async def explore():
    global results
    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True)
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36')
        page = await context.new_page()
        
        urls = ['https://betboom.ru/live/football', 'https://betboom.ru/live', 'https://betboom.ru/']
        
        for url in urls:
            result = {'url': url}
            try:
                await page.goto(url, wait_until='domcontentloaded', timeout=30000)
                await asyncio.sleep(5)
                result['title'] = await page.title()
                html = await page.content()
                result['html_len'] = len(html)
                
                counters = {}
                for sel in ['event', 'match', 'bm-event', 'odds', 'coef', 'rate', 'button', 'bet']:
                    try:
                        cnt = await page.locator('.' + sel).count()
                        if cnt > 0:
                            counters[sel] = cnt
                    except: pass
                result['counters'] = counters
            except Exception as e:
                result['error'] = str(e)
            
            results.append(result)
        
        await browser.close()
        
        # Write results to file
        with open('betboom_results.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)

asyncio.run(explore())
print('Done')
