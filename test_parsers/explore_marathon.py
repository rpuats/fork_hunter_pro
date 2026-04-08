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
        
        # Try different Marathon URLs
        urls = [
            'https://www.marathonbet.com/ru/live/football/',
            'https://www.marathonbet.com/ru/live/',
            'https://www.marathonbet.com/'
        ]
        
        for url in urls:
            result = {'url': url}
            try:
                await page.goto(url, wait_until='domcontentloaded', timeout=30000)
                await asyncio.sleep(8)
                
                result['title'] = await page.title()
                
                # Find any class patterns
                classes = await page.evaluate('''
                    () => {
                        const all = document.querySelectorAll('*');
                        const classes = new Set();
                        all.forEach(el => {
                            if (el.className && typeof el.className === 'string') {
                                el.className.split(' ').forEach(c => {
                                    if (c.length > 3 && !c.match(/^(active|disabled|open|close|visible|hidden)$/i)) {
                                        classes.add(c);
                                    }
                                });
                            }
                        });
                        return Array.from(classes).slice(0, 50);
                    }
                ''')
                result['classes'] = classes
                
                # Find odds-like numbers
                odds = await page.evaluate('''
                    () => {
                        const odds = [];
                        const all = document.querySelectorAll('*');
                        all.forEach(el => {
                            const txt = el.textContent.trim();
                            if (/^\\d+\\.\\d{2}$/.test(txt) && parseFloat(txt) > 1 && parseFloat(txt) < 50) {
                                odds.push({
                                    class: el.className,
                                    tag: el.tagName,
                                    text: txt,
                                    parent: el.parentElement?.className || 'none'
                                });
                            }
                        });
                        return odds.slice(0, 30);
                    }
                ''')
                result['odds'] = odds
                
            except Exception as e:
                result['error'] = str(e)
            
            results.append(result)
        
        await browser.close()
        
        with open('marathon_results.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Done')

asyncio.run(explore())
