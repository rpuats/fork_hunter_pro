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
        await asyncio.sleep(8)
        
        result = {}
        
        # Try to find events using bb-KI container
        events = await page.evaluate('''
            () => {
                const events = [];
                const containers = document.querySelectorAll('.bb-KI, .bb-Nm, .bb-Om, .bb-LI, .bb-mK, .bb-jK');
                
                console.log('Found containers: ' + containers.length);
                
                containers.forEach((container, idx) => {
                    try {
                        const html = container.outerHTML;
                        // Find team-like text
                        const text = container.textContent.trim();
                        // Find odds-like values
                        const odds = [];
                        const all = container.querySelectorAll('*');
                        all.forEach(el => {
                            const txt = el.textContent.trim();
                            if (/^\\d+\\.\\d{2}$/.test(txt) && parseFloat(txt) > 1 && parseFloat(txt) < 50) {
                                odds.push({class: el.className, text: txt, tag: el.tagName});
                            }
                        });
                        
                        if (odds.length >= 2) {
                            events.push({
                                container_class: container.className,
                                text_sample: text.substring(0, 100),
                                odds_count: odds.length,
                                odds_sample: odds.slice(0, 6)
                            });
                        }
                    } catch(e) {}
                });
                return events.slice(0, 30);
            }
        ''')
        result['events'] = events
        
        results.append(result)
        
        await browser.close()
        
        with open('betboom_results5.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Found: ' + str(len(events)) + ' events with odds')
        print('Done')

asyncio.run(explore())
