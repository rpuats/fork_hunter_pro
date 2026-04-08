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
        
        # Get more complete event structure - look for parent containers that might have teams
        events = await page.evaluate('''
            () => {
                const events = [];
                // Look for event rows that contain bb-Nm (which has odds)
                const nmContainers = document.querySelectorAll('.bb-Nm');
                
                nmContainers.forEach((container, idx) => {
                    try {
                        // Get the full text content
                        const text = container.textContent.trim();
                        
                        // Find parent chain to locate teams
                        let parent = container.parentElement;
                        let grandparent = parent?.parentElement;
                        let greatGrandparent = grandparent?.parentElement;
                        
                        // Look for team names in parents
                        let teamText = '';
                        if (greatGrandparent) {
                            teamText = greatGrandparent.textContent.trim();
                        } else if (grandparent) {
                            teamText = grandparent.textContent.trim();
                        }
                        
                        // Extract teams from team text
                        const teams = teamText.split(/\\n|\\r/).filter(t => t.trim().length > 3 && t.trim().length < 40);
                        
                        // Get all odds
                        const odds = [];
                        container.querySelectorAll('.bb-Rm').forEach(o => {
                            const txt = o.textContent.trim();
                            if (/^\\d+\\.\\d{2}$/.test(txt)) {
                                odds.push(txt);
                            }
                        });
                        
                        if (odds.length >= 2) {
                            events.push({
                                team_text: teamText.substring(0, 200),
                                teams: teams.slice(0, 4),
                                odds: odds,
                                parent_classes: parent?.className || 'none',
                                grandparent_classes: grandparent?.className || 'none'
                            });
                        }
                    } catch(e) {}
                });
                return events.slice(0, 15);
            }
        ''')
        result['events'] = events
        
        # Also try to find any elements that look like they contain team names
        team_elements = await page.evaluate('''
            () => {
                const elements = [];
                // Try to find any elements containing team-like patterns
                const all = document.querySelectorAll('*');
                all.forEach(el => {
                    const txt = el.textContent.trim();
                    // Look for patterns like "Team - Team" or "Team vs Team"
                    if (txt.match(/^[A-ZА-Я][a-zа-я].+\\s[-–—]\\s.+/)) {
                        elements.push({
                            tag: el.tagName,
                            class: el.className,
                            text: txt.substring(0, 80)
                        });
                    }
                });
                return elements.slice(0, 15);
            }
        ''')
        result['team_elements'] = team_elements
        
        results.append(result)
        
        await browser.close()
        
        with open('betboom_results6.json', 'w', encoding='utf-8') as f:
            json.dump(results, f, ensure_ascii=False, indent=2)
        print('Done')

asyncio.run(explore())
