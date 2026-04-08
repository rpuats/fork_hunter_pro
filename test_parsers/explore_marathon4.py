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
        
        await page.goto('https://www.marathonbet.ru/live/', wait_until='domcontentloaded', timeout=30000)
        await asyncio.sleep(10)
        
        # Find events with team names
        events = await page.evaluate('''
            () => {
                const events = [];
                
                // Find event containers - try different selectors
                const containers = document.querySelectorAll('.event-line, .sport-event, [class*=\"event\"], [class*=\"runner\"]');
                
                // Alternative: find parent elements that contain both team names and odds
                const allElements = document.querySelectorAll('*');
                const eventRows = [];
                
                allElements.forEach(el => {
                    const txt = el.textContent;
                    // Look for elements containing both team patterns and odds
                    if (txt.match(/\\d+\\.\\d{2}/) && txt.match(/[A-ZА-Я][a-zа-я]+/)) {
                        const hasPrice = el.querySelector('.price, .selection-link');
                        if (hasPrice) {
                            eventRows.push(el);
                        }
                    }
                });
                
                console.log('Found event rows: ' + eventRows.length);
                
                // Try more targeted approach - look for elements with both team names and prices
                const priceElements = document.querySelectorAll('.price, .selection-link');
                console.log('Found price elements: ' + priceElements.length);
                
                priceElements.forEach(price => {
                    const container = price.closest('[class*=\"row\"], [class*=\"event\"], [class*=\"game\"], [class*=\"match\"]');
                    if (container) {
                        const text = container.textContent.trim();
                        const odds = [];
                        container.querySelectorAll('.price').forEach(o => {
                            const txt = o.textContent.trim();
                            if (/^\\d+\\.\\d{2}$/.test(txt)) {
                                odds.push(txt);
                            }
                        });
                        
                        if (odds.length >= 2) {
                            events.push({
                                class: container.className,
                                text_sample: text.substring(0, 200),
                                odds: odds.slice(0, 6)
                            });
                        }
                    }
                });
                
                return events.slice(0, 20);
            }
        ''')
        result = {'events': events}
        
        results.append(result)
        
        await browser.close()
        
        with open('marathon_results4.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Found: ' + str(len(events)) + ' events')

asyncio.run(explore())
