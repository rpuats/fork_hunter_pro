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
            locale='ru-RU',
            extra_http_headers={
                'Accept-Language': 'ru-RU,ru;q=0.9'
            }
        )
        page = await context.new_page()
        
        # Try Russian version
        urls = [
            'https://www.marathonbet.ru/ru/live/football/',
            'https://www.marathonbet.ru/live/',
        ]
        
        for url in urls:
            result = {'url': url}
            try:
                await page.goto(url, wait_until='domcontentloaded', timeout=30000)
                await asyncio.sleep(10)
                
                html = await page.content()
                result['html_length'] = len(html)
                
                # Check for odds
                odds = await page.evaluate('''
                    () => {
                        const odds = [];
                        const all = document.querySelectorAll('*');
                        all.forEach(el => {
                            const txt = el.textContent.trim();
                            if (/^\\d+\\.\\d{2}$/.test(txt) && parseFloat(txt) > 1 && parseFloat(txt) < 50) {
                                odds.push({
                                    class: el.className,
                                    text: txt
                                });
                            }
                        });
                        return odds.slice(0, 20);
                    }
                ''')
                result['odds'] = odds
                
            except Exception as e:
                result['error'] = str(e)
            
            results.append(result)
        
        await browser.close()
        
        with open('marathon_results3.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Done')

asyncio.run(explore())
