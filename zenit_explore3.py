import asyncio
from playwright.async_api import async_playwright

async def explore():
    results = []
    results.append('=== Zenit Deep Exploration ===')
    
    pw = await async_playwright().start()
    
    try:
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled'])
        page = await browser.new_page()
        
        await page.set_viewport_size({'width': 1920, 'height': 1080})
        
        results.append('1. Navigating to https://zenit.win/live...')
        await page.goto('https://zenit.win/live', wait_until='domcontentloaded', timeout=30000)
        
        results.append('2. Waiting 10 seconds...')
        await asyncio.sleep(10)
        
        results.append('')
        results.append('3. Trying to extract events using JavaScript...')
        
        events_data = await page.evaluate('''
            () => {
                const results = [];
                
                // Try to find all divs that contain odds (numbers between 1.01 and 50)
                const allDivs = document.querySelectorAll('div');
                
                // Get all text content and look for patterns
                const bodyText = document.body.innerText;
                
                // Look for match-like patterns: Team1 vs Team2
                const lines = bodyText.split('\\n').map(l => l.trim()).filter(l => l);
                
                let currentEvent = null;
                
                // Zenit seems to use tabular layout - odds are in columns
                // Let's look for patterns with 3 numbers (1x2 odds)
                
                for (let i = 0; i < lines.length; i++) {
                    const line = lines[i];
                    // Check if line contains 3 odds-like numbers
                    const oddsMatches = line.match(/(\\d+[.,]\\d+)\\s+(\\d+[.,]\\d+)\\s+(\\d+[.,]\\d+)/);
                    
                    if (oddsMatches && line.length < 50) {
                        // This looks like odds line
                        const odds = oddsMatches.slice(1).map(o => parseFloat(o.replace(',', '.')));
                        
                        // Try to get teams from nearby lines
                        let home = '';
                        let away = '';
                        for (let j = i - 1; j >= 0 && j >= i - 5; j--) {
                            const prevLine = lines[j];
                            if (prevLine.length > 2 && prevLine.length < 40 && !prevLine.match(/^\\d+/) && !prevLine.match(/:/)) {
                                if (!home) home = prevLine;
                                else if (prevLine !== home) {
                                    away = prevLine;
                                    break;
                                }
                            }
                        }
                        
                        if (home && away && odds[0] >= 1.01 && odds[0] <= 50) {
                            results.push({home, away, odds});
                        }
                    }
                }
                
                return results.slice(0, 20);
            }
        ''')
        
        results.append(f'   Found {len(events_data)} potential events')
        
        for i, e in enumerate(events_data[:10]):
            results.append(f'   Event {i+1}: {e[\
home\]} vs {e[\away\]} -> {e[\odds\]}')
        
        results.append('')
        results.append('4. Trying to find CSS classes from page...')
        
        classes = await page.evaluate('''
            () => {
                const classes = new Set();
                document.querySelectorAll('*').forEach(el => {
                    el.classList.forEach(c => {
                        if (c.length > 2 && c.length < 30) classes.add(c);
                    });
                });
                return Array.from(classes).slice(0, 100);
            }
        ''')
        
        results.append('   Unique CSS classes found:')
        for c in classes:
            results.append(f'     .{c}')
        
        results.append('')
        results.append('5. Checking for tables/rows structure...')
        
        structure = await page.evaluate('''
            () => {
                const tables = document.querySelectorAll('table');
                const divs = document.querySelectorAll('div');
                
                return {
                    tables: tables.length,
                    divs: divs.length,
                    bodyHTML: document.body.innerHTML.substring(0, 5000)
                };
            }
        ''')
        
        results.append(f'   Tables: {structure[\tables\]}, Divs: {structure[\divs\]}')
        
        await browser.close()
    except Exception as e:
        results.append(f'Error: {e}')
        try:
            await browser.close()
        except:
            pass
    
    await pw.stop()
    
    with open('zenit_results2.txt', 'w', encoding='utf-8') as f:
        f.write('\\n'.join(results))
    print('Done')

asyncio.run(explore())
