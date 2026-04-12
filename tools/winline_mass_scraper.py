"""Winline mass scraper - ALL sports, parallel, optimized for speed"""
import asyncio
import json
import sys
import os
import time

async def scrape_page(browser, url, is_live, seen, results):
    """Scrape single page - fast and simple"""
    try:
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
        )
        page = await context.new_page()
        
        # Fast load
        await page.goto(url, wait_until='domcontentloaded', timeout=15000)
        await page.wait_for_timeout(2000)
        
        # Quick scroll
        await page.evaluate('window.scrollTo(0, document.body.scrollHeight)')
        await page.wait_for_timeout(1500)
        
        # Extract events - SIMPLE approach
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
                results.append({
                    'home': ev['home'],
                    'away': ev['away'],
                    'odds': ev['odds'],
                    'bookmaker': 'winline',
                    'is_live': is_live,
                    'league': '',
                })
                count += 1
        
        await page.close()
        await context.close()
        return count
    except Exception as e:
        return 0

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
            args=['--no-sandbox', '--disable-dev-shm-usage', '--disable-web-security']
        )
        
        seen = set()
        results = []
        
        # Process in parallel batches of 4
        batch_size = 4
        for i in range(0, len(urls), batch_size):
            batch = urls[i:i+batch_size]
            tasks = [scrape_page(browser, url, is_live, seen, results) for url, is_live in batch]
            counts = await asyncio.gather(*tasks, return_exceptions=True)
            total_so_far = len(results)
            print(f"  Batch {i//batch_size + 1}: +{sum(c for c in counts if isinstance(c, int))} events, total: {total_so_far}", file=sys.stderr)
        
        await browser.close()
    
    elapsed = time.time() - start_time
    print(f"Total: {len(results)} events in {elapsed:.1f}s", file=sys.stderr)
    
    # Save
    output = {
        "bookmaker": "winline",
        "events": results,
        "count": len(results),
        "scrape_time": elapsed,
    }
    
    output_file = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'winline_events.json')
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(output, f, ensure_ascii=False, default=str)
    
    print(f"Saved {len(results)} events to winline_events.json", file=sys.stderr)
    print(json.dumps(output, ensure_ascii=False, default=str))

if __name__ == "__main__":
    asyncio.run(main())
