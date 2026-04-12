"""Winline scraper - sequential, all sports, proven working approach"""
import asyncio
import json
import sys
import os
import time

async def main():
    from playwright.async_api import async_playwright
    
    urls = [
        ("https://winline.ru/football", False),
        ("https://winline.ru/live/football", True),
        ("https://winline.ru/basketball", False),
        ("https://winline.ru/live/basketball", True),
        ("https://winline.ru/hockey", False),
        ("https://winline.ru/live/hockey", True),
        ("https://winline.ru/tennis", False),
        ("https://winline.ru/live/tennis", True),
        ("https://winline.ru/volleyball", False),
        ("https://winline.ru/live/volleyball", True),
        ("https://winline.ru/table-tennis", False),
        ("https://winline.ru/live/table-tennis", True),
        ("https://winline.ru/baseball", False),
        ("https://winline.ru/live/baseball", True),
        ("https://winline.ru/handball", False),
        ("https://winline.ru/live/handball", True),
        ("https://winline.ru/cyber-sport", False),
        ("https://winline.ru/live/cyber-sport", True),
        ("https://winline.ru/rugby", False),
        ("https://winline.ru/live/rugby", True),
        ("https://winline.ru/badminton", False),
        ("https://winline.ru/live/badminton", True),
    ]
    
    print(f"Starting Winline scraper - {len(urls)} pages...", file=sys.stderr)
    start_time = time.time()
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=['--no-sandbox', '--disable-dev-shm-usage']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
        )
        
        all_events = []
        seen = set()
        
        for url, is_live in urls:
            try:
                page = await context.new_page()
                await page.goto(url, wait_until='domcontentloaded', timeout=15000)
                await page.wait_for_timeout(2500)
                await page.evaluate('window.scrollTo(0, document.body.scrollHeight / 2)')
                await page.wait_for_timeout(1500)
                await page.evaluate('window.scrollTo(0, document.body.scrollHeight)')
                await page.wait_for_timeout(1500)
                
                events = await page.evaluate('''
                    () => {
                        const events = [];
                        const allEls = document.querySelectorAll('*');
                        for (const el of allEls) {
                            try {
                                const text = el.textContent || '';
                                if (text.length < 20 || text.length > 2000) continue;
                                const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 2 && l.length < 60);
                                if (lines.length < 3) continue;
                                const teams = [];
                                const odds = [];
                                for (const line of lines) {
                                    const val = parseFloat(line.replace(',', '.'));
                                    if (!isNaN(val) && val >= 1.01 && val <= 50) {
                                        odds.push(val);
                                    } else if (!line.match(/^\\d+[.,]\\d+$/) && !line.match(/^\\d{1,2}:\\d{2}/) && !line.match(/LIVE/i) && line.length > 2) {
                                        if (!teams.includes(line)) teams.push(line);
                                    }
                                    if (teams.length >= 2 && odds.length >= 3) break;
                                }
                                if (teams.length >= 2 && odds.length >= 3) {
                                    events.push({home: teams[0], away: teams[1] || teams[teams.length-1], odds: odds.slice(0, 5)});
                                }
                            } catch(e) {}
                        }
                        return events;
                    }
                ''')
                
                count = 0
                for ev in events:
                    key = f"{ev['home']}|{ev['away']}"
                    if key not in seen:
                        seen.add(key)
                        all_events.append({
                            'home': ev['home'],
                            'away': ev['away'],
                            'odds': ev['odds'],
                            'bookmaker': 'winline',
                            'is_live': is_live,
                            'league': '',
                        })
                        count += 1
                
                await page.close()
                print(f"  {url}: +{count} unique, total: {len(all_events)}", file=sys.stderr)
                
            except Exception as e:
                print(f"  {url}: Error: {str(e)[:80]}", file=sys.stderr)
        
        await browser.close()
    
    elapsed = time.time() - start_time
    print(f"Total: {len(all_events)} events in {elapsed:.1f}s", file=sys.stderr)
    
    output = {
        "bookmaker": "winline",
        "events": all_events,
        "count": len(all_events),
    }
    
    output_file = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'winline_events.json')
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(output, f, ensure_ascii=False, default=str)
    
    print(f"Saved {len(all_events)} events", file=sys.stderr)
    print(json.dumps(output, ensure_ascii=False, default=str))

if __name__ == "__main__":
    asyncio.run(main())
