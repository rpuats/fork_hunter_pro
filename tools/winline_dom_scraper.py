"""Winline DOM scraper - extracts events from rendered page"""
import asyncio
import json
import sys
import time

async def main():
    from playwright.async_api import async_playwright
    
    urls = [
        "https://winline.ru/football",
        "https://winline.ru/live/football",
        "https://winline.ru/basketball",
        "https://winline.ru/live/basketball",
        "https://winline.ru/hockey",
        "https://winline.ru/live/hockey",
    ]
    
    all_events = []
    seen = set()
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        # ONE context для сохранения cookies
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
        )
        
        for i, url in enumerate(urls):
            print(f"[{i+1}/{len(urls)}] {url}...", file=sys.stderr)
            page = await context.new_page()
            
            try:
                await page.goto(url, wait_until='domcontentloaded', timeout=15000)
                await page.wait_for_timeout(3000)
                await page.evaluate('window.scrollTo(0, document.body.scrollHeight)')
                await page.wait_for_timeout(2000)
                
                # Extract events
                events = await page.evaluate('''
                    () => {
                        const events = [];
                        document.querySelectorAll('*').forEach(el => {
                            try {
                                const text = el.textContent || '';
                                if (text.length < 20 || text.length > 2000) return;
                                const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 2);
                                const teams = [];
                                const odds = [];
                                for (const line of lines) {
                                    const val = parseFloat(line.replace(',', '.'));
                                    if (!isNaN(val) && val >= 1.01 && val <= 50) odds.push(val);
                                    else if (!line.match(/^\\d+[.,]\\d+$/) && !line.match(/^\\d{1,2}:\\d{2}/) && !line.match(/LIVE/i) && line.length > 2) {
                                        if (!teams.includes(line)) teams.push(line);
                                    }
                                    if (teams.length >= 2 && odds.length >= 3) break;
                                }
                                if (teams.length >= 2 && odds.length >= 3) {
                                    events.push({home: teams[0], away: teams[1], odds: odds.slice(0, 5)});
                                }
                            } catch(e) {}
                        });
                        return events;
                    }
                ''')
                
                count = 0
                for ev in events:
                    key = f"{ev['home']}|{ev['away']}"
                    if key not in seen and len(ev['odds']) >= 3:
                        seen.add(key)
                        all_events.append({
                            'home_team': ev['home'],
                            'away_team': ev['away'],
                            'odds': ev['odds'],
                            'bookmaker': 'winline',
                            'is_live': 'live' in url,
                            'league': '',
                        })
                        count += 1
                
                await page.close()
                print(f"  +{count} events, total: {len(all_events)}", file=sys.stderr)
                
            except Exception as e:
                print(f"  Error: {str(e)[:80]}", file=sys.stderr)
                await page.close()
        
        await browser.close()
    
    print(f"Total: {len(all_events)} events", file=sys.stderr)
    
    # Save
    output = {"bookmaker": "winline", "events": all_events, "count": len(all_events)}
    with open('winline_events_final.json', 'w', encoding='utf-8') as f:
        json.dump(output, f, ensure_ascii=False, default=str)
    
    print(json.dumps(output, ensure_ascii=False, default=str))

if __name__ == "__main__":
    asyncio.run(main())
