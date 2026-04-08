"""
Direct Playwright bridge - no parser classes, just raw browser automation.
Usage: python direct_bridge.py <parser> <url>
Output: JSON array to stdout
"""
import asyncio
import json
import sys
import os
import time

# Setup paths
script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
if project_root not in sys.path:
    sys.path.insert(0, project_root)

from playwright.async_api import async_playwright
from scanner.parsers.stealth import create_stealth_context, generate_stealth_config

EXTRACT_JS = """
() => {
    const events = [];
    const isValidName = (t) => {
        if (!t || t.length < 2 || t.length > 80) return false;
        if (t === '-' || /^[-\\s]+$/.test(t)) return false;
        if (/^(event|match|game|live|pre)/i.test(t)) return false;
        return true;
    };
    const cards = document.querySelectorAll('ww-feature-event-mini-card-dsk');
    cards.forEach(card => {
        try {
            let home = '';
            let away = '';
            const nameEls = card.querySelectorAll('.half__names .name');
            if (nameEls.length >= 2) {
                home = (nameEls[0].getAttribute('title') || nameEls[0].textContent || '').trim();
                away = (nameEls[1].getAttribute('title') || nameEls[1].textContent || '').trim();
                if (home && home === away && home.includes(' - ')) {
                    const parts = home.split(' - ');
                    if (parts.length >= 2) { home = parts[0].trim(); away = parts[1].trim(); }
                }
            }
            if (!isValidName(home) || !isValidName(away)) return;
            const odds = [];
            const coefBtns = card.querySelectorAll('.half__coef-buttons .button__coef-title');
            coefBtns.forEach(btn => {
                const text = btn.textContent.trim();
                const val = parseFloat(text.replace(',', '.'));
                if (!isNaN(val) && val >= 1.01 && val <= 100) {
                    odds.push(val);
                }
            });
            if (odds.length >= 2) {
                events.push({ home, away, odds: odds.slice(0, 3) });
            }
        } catch(e) {}
    });
    return events;
}
"""

async def extract_events(url, parser_name):
    sys.stderr.write(f"BRIDGE: Starting {parser_name} at {url}\n")
    sys.stderr.flush()
    
    pw = await async_playwright().start()
    browser = await pw.chromium.launch(
        headless=True,
        args=[
            '--disable-blink-features=AutomationControlled',
            '--no-sandbox',
            '--disable-dev-shm-usage',
            '--disable-web-security',
            '--disable-features=IsolateOrigins,site-per-process',
            '--disable-infobars',
        ]
    )
    
    config = generate_stealth_config()
    context = await create_stealth_context(browser, config)
    page = await context.new_page()
    
    try:
        sys.stderr.write(f"BRIDGE: Navigating to {url}\n")
        sys.stderr.flush()
        
        await page.goto(url, wait_until='networkidle', timeout=120000)
        await asyncio.sleep(10)
        
        card_count = await page.evaluate("document.querySelectorAll('ww-feature-event-mini-card-dsk').length")
        sys.stderr.write(f"BRIDGE: Found {card_count} custom element cards\n")
        sys.stderr.flush()
        
        body_len = await page.evaluate("(document.body.innerText || '').length")
        sys.stderr.write(f"BRIDGE: Body text length: {body_len}\n")
        sys.stderr.flush()
        
        events_data = await page.evaluate(EXTRACT_JS)
        sys.stderr.write(f"BRIDGE: JS returned {len(events_data)} events\n")
        sys.stderr.flush()
        
        result = []
        for e in events_data:
            home = e.get('home', '')
            away = e.get('away', '')
            odds = e.get('odds', [])
            if not home or not away or len(odds) < 2:
                continue
            if len(odds) == 2:
                result.append({
                    'home_team': home, 'away_team': away, 'league': '',
                    'home_odds': None, 'draw_odds': None, 'away_odds': None, 'is_live': 'live' in url
                })
            elif len(odds) >= 3:
                result.append({
                    'home_team': home, 'away_team': away, 'league': '',
                    'home_odds': odds[0], 'draw_odds': odds[1], 'away_odds': odds[2], 'is_live': 'live' in url
                })
        
        sys.stderr.write(f"BRIDGE: Returning {len(result)} events\n")
        sys.stderr.flush()
        
        print(json.dumps(result, ensure_ascii=False))
        sys.stdout.flush()
        
    except Exception as ex:
        sys.stderr.write(f"BRIDGE_ERROR: {ex}\n")
        sys.stderr.flush()
        print(json.dumps([]))
        sys.stdout.flush()
    finally:
        await page.close()
        await context.close()
        await browser.close()
        await pw.stop()

async def main():
    if len(sys.argv) < 3:
        print(json.dumps([]))
        sys.exit(0)
    
    parser_name = sys.argv[1]
    url = sys.argv[2]
    
    # Winline uses custom web components
    if parser_name in ['winline', 'zenit', 'betcity', 'baltbet']:
        await extract_events(url, parser_name)
    else:
        print(json.dumps([]))

if __name__ == '__main__':
    asyncio.run(main())
