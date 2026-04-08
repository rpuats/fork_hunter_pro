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
        await asyncio.sleep(3)
        
        result = {}
        
        # Find team names
        teams = await page.evaluate('''
            () => {
                const teamElements = document.querySelectorAll('[class*=\"team\"], [class*=\"comp\"], [class*=\"name\"], [class*=\"member\"], .bb-Nm');
                const teams = [];
                teamElements.forEach(el => {
                    const txt = el.textContent.trim();
                    if (txt.length > 2 && txt.length < 40 && !txt.match(/\\d/)) {
                        teams.push({
                            class: el.className,
                            text: txt
                        });
                    }
                });
                return teams.slice(0, 30);
            }
        ''')
        result['teams'] = teams
        
        # Find event container structure - look for containers with odds
        event_structure = await page.evaluate('''
            () => {
                const containers = [];
                // Find containers that have bb-OI inside
                const oddsElements = document.querySelectorAll('.bb-OI');
                oddsElements.forEach(odd => {
                    const parent = odd.closest('[class*=\"bb-\"]');
                    if (parent) {
                        containers.push({
                            odd_class: odd.className,
                            parent_class: parent.className,
                            grandparent_class: parent.parentElement?.className || 'none'
                        });
                    }
                });
                return containers.slice(0, 20);
            }
        ''')
        result['event_structure'] = event_structure
        
        # Find complete events - look for containers with team names and odds
        complete_events = await page.evaluate('''
            () => {
                const events = [];
                // Find all bb-KI containers that likely represent events
                const containers = document.querySelectorAll('.bb-KI');
                containers.forEach((container, idx) => {
                    try {
                        const odds = [];
                        container.querySelectorAll('.bb-OI').forEach(o => {
                            const txt = o.textContent.trim();
                            if (/^\\d+\\.\\d{2}$/.test(txt)) {
                                odds.push(txt);
                            }
                        });
                        
                        // Look for team names - they might be in different locations
                        let home = '', away = '';
                        const texts = container.querySelectorAll('[class*=\"Nm\"], [class*=\"name\"], [class*=\"team\"]');
                        texts.forEach(t => {
                            const txt = t.textContent.trim();
                            if (txt.length > 2 && txt.length < 30) {
                                if (!home) home = txt;
                                else if (!away) away = txt;
                            }
                        });
                        
                        if (odds.length >= 2) {
                            events.push({home, away, odds: odds.slice(0, 3)});
                        }
                    } catch(e) {}
                });
                return events.slice(0, 20);
            }
        ''')
        result['complete_events'] = complete_events
        
        results.append(result)
        
        await browser.close()
        
        with open('betboom_results3.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Done')

asyncio.run(explore())
