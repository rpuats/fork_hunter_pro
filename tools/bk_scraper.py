"""Universal BK scraper - loads page via Playwright and saves events as JSON"""
import asyncio
import json
import sys
import os
import time

BK_CONFIGS = {
    "winline": {
        "url": "https://winline.ru/football",
        "selector": "ww-feature-event-mini-card-dsk",
        "js_extract": """
            () => {
                const events = [];
                const cards = document.querySelectorAll('ww-feature-event-mini-card-dsk');
                cards.forEach(card => {
                    try {
                        let home = '', away = '';
                        const nameEls = card.querySelectorAll('.half__names .name');
                        if (nameEls.length >= 2) {
                            home = (nameEls[0].getAttribute('title') || nameEls[0].textContent || '').trim();
                            away = (nameEls[1].getAttribute('title') || nameEls[1].textContent || '').trim();
                            if (home && home === away && home.includes(' - ')) {
                                const parts = home.split(' - ');
                                if (parts.length >= 2) { home = parts[0].trim(); away = parts[1].trim(); }
                            }
                        }
                        if (!home || !away || home.length < 2) return;
                        const odds = [];
                        card.querySelectorAll('.half__coef-buttons .button__coef-title').forEach(btn => {
                            const val = parseFloat(btn.textContent.trim().replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 100) odds.push(val);
                        });
                        if (odds.length >= 2) events.push({home, away, odds: odds.slice(0, 5)});
                    } catch(e) {}
                });
                return events;
            }
        """,
        "urls_extra": [
            "https://winline.ru/live/football",
            "https://winline.ru/basketball",
            "https://winline.ru/hockey",
        ]
    },
    "zenit": {
        "url": "https://zenit.win/line/football",
        "selector": "[class*='event'], [class*='match']",
        "js_extract": """
            () => {
                const events = [];
                const containers = document.querySelectorAll('[class*="event"], [class*="match"], .sport-event, .event-item');
                containers.forEach(el => {
                    try {
                        const text = el.textContent || '';
                        if (!text || text.length < 10) return;
                        const lines = text.split(/\\n/).filter(l => l.trim());
                        const teams = [];
                        const odds = [];
                        for (const line of lines) {
                            const clean = line.trim();
                            if (clean.length > 2 && clean.length < 50 &&
                                !clean.match(/^\\d+[.,]\\d+$/) && !clean.match(/^\\d{1,2}:\\d{2}/) && !clean.match(/^LIVE$/i)) {
                                if (teams.length === 0 || teams[teams.length - 1] !== clean) teams.push(clean);
                            }
                            const val = parseFloat(clean.replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 50) odds.push(val);
                            if (teams.length >= 3 && odds.length >= 2) break;
                        }
                        const home = teams[0] || '';
                        const away = teams.length > 1 ? teams[teams.length - 1] : (teams[1] || '');
                        if (home && away && odds.length >= 2) events.push({home, away, odds: odds.slice(0, 5)});
                    } catch(e) {}
                });
                return events;
            }
        """,
        "urls_extra": [
            "https://zenit.win/live/football",
            "https://zenit.win/line/basketball",
            "https://zenit.win/live/basketball",
        ]
    },
    "betcity": {
        "url": "https://betcity.ru/ru/line/football",
        "selector": ".line-event",
        "js_extract": """
            () => {
                const events = [];
                document.querySelectorAll('.line-event').forEach(el => {
                    try {
                        const nameTexts = el.querySelectorAll('.line-event__name-text');
                        const teams = [];
                        nameTexts.forEach(nt => { const t = nt.textContent.trim(); if (t) teams.push(t); });
                        const odds = [];
                        el.querySelectorAll('.line-event__main-bets-button').forEach(btn => {
                            const val = parseFloat(btn.textContent.trim().replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 100) odds.push(val);
                        });
                        if (teams.length >= 2 && odds.length >= 2) events.push({home: teams[0], away: teams[1], odds: odds.slice(0, 5)});
                    } catch(e) {}
                });
                return events;
            }
        """,
        "urls_extra": [
            "https://betcity.ru/ru/live/football",
            "https://betcity.ru/ru/line/basketball",
        ]
    },
    "baltbet": {
        "url": "https://baltbet.ru/line",
        "selector": "[class*='event'], [class*='match']",
        "js_extract": """
            () => {
                const events = [];
                document.querySelectorAll('[class*="event"], [class*="match"], .sport-event, .event-line').forEach(el => {
                    try {
                        const text = el.textContent || '';
                        if (!text || text.length < 20) return;
                        const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                        const teams = [];
                        const odds = [];
                        for (const line of lines) {
                            const val = parseFloat(line.replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 50) odds.push(val);
                            else if (line.length > 2 && line.length < 40 && !line.match(/LIVE|live/i)) teams.push(line);
                            if (teams.length >= 2 && odds.length >= 1) break;
                        }
                        if (teams.length >= 2 && odds.length >= 2) events.push({home: teams[0], away: teams[1], odds: odds.slice(0, 5)});
                    } catch(e) {}
                });
                return events;
            }
        """,
        "urls_extra": [
            "https://baltbet.ru/live",
            "https://baltbet.ru/line/basketball",
        ]
    },
}

async def scrape_bk(bk_name):
    from playwright.async_api import async_playwright
    
    config = BK_CONFIGS.get(bk_name)
    if not config:
        print(f"Unknown BK: {bk_name}")
        return []
    
    all_events = []
    seen = set()
    
    urls = [config["url"]] + config.get("urls_extra", [])
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=[
                '--no-sandbox',
                '--disable-blink-features=AutomationControlled',
                '--disable-dev-shm-usage',
                '--disable-web-security',
                '--disable-features=IsolateOrigins,site-per-process',
            ]
        )
        
        from playwright_stealth import Stealth
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        Stealth().apply_stealth_sync(context)
        
        for url in urls:
            try:
                page = await context.new_page()
                await page.goto(url, wait_until='domcontentloaded', timeout=20000)
                await page.wait_for_timeout(3000)
                
                # Scroll to load lazy content
                await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
                await page.wait_for_timeout(1000)
                await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                await page.wait_for_timeout(1000)
                
                # Extract events
                events = await page.evaluate(config["js_extract"])
                
                for ev in events:
                    key = f"{ev.get('home','')}|{ev.get('away','')}"
                    if key not in seen and ev.get('home') and ev.get('away'):
                        seen.add(key)
                        ev['bookmaker'] = bk_name
                        all_events.append(ev)
                
                await page.close()
                print(f"  {bk_name} {url}: +{len(events)} raw, total: {len(all_events)}", file=sys.stderr)
            except Exception as e:
                print(f"  {bk_name} {url}: Error: {e}", file=sys.stderr)
        
        await browser.close()
    
    print(f"{bk_name}: {len(all_events)} events scraped", file=sys.stderr)
    return all_events

async def main():
    bk_name = sys.argv[1] if len(sys.argv) > 1 else "winline"
    events = await scrape_bk(bk_name)
    
    # Output as JSON
    output = {
        "bookmaker": bk_name,
        "events": events,
        "count": len(events),
        "timestamp": time.time()
    }
    print(json.dumps(output, ensure_ascii=False, default=str))

if __name__ == "__main__":
    asyncio.run(main())
