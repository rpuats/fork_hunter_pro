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
        
        await page.goto('https://betboom.ru/', wait_until='domcontentloaded', timeout=30000)
        # Wait longer for JS to render
        await asyncio.sleep(8)
        
        result = {}
        
        # Get page HTML
        html = await page.content()
        result['html_sample'] = html[0:50000]
        
        # Try to find any elements with bb- prefix classes
        bb_classes = await page.evaluate('''
            () => {
                const all = document.querySelectorAll('*');
                const classes = new Set();
                all.forEach(el => {
                    if (el.className && typeof el.className === 'string') {
                        el.className.split(' ').forEach(c => {
                            if (c.startsWith('bb-')) {
                                classes.add(c);
                            }
                        });
                    }
                });
                return Array.from(classes);
            }
        ''')
        result['bb_classes'] = bb_classes
        
        # Try querySelectorAll on found classes
        for cls in bb_classes[:10]:
            cnt = await page.locator('.' + cls).count()
            print('.' + cls + ': ' + str(cnt))
        
        results.append(result)
        
        await browser.close()
        
        with open('betboom_results4.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Done')

asyncio.run(explore())
