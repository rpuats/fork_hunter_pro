"""
Bridge script: calls Playwright parser and outputs JSON for Rust.
Usage: python parser_bridge.py <parser_name> <url>
"""
import asyncio
import json
import sys
import os
import traceback
from playwright.async_api import async_playwright

# Add project root to sys.path
script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.abspath(os.path.join(script_dir, '..', '..'))
if project_root not in sys.path:
    sys.path.insert(0, project_root)

def log(msg):
    sys.stderr.write(f"BRIDGE: {msg}\n")
    sys.stderr.flush()

async def run_parser(parser_name, url):
    log(f"Starting {parser_name} parser for {url}")
    log(f"Python path: {sys.path[:3]}")
    
    try:
        if parser_name == 'winline':
            from scanner.parsers.winline_playwright import WinlinePlaywrightParser
            log("Imported WinlinePlaywrightParser")
            
            # Use the parser exactly like test_parser() does
            pw_module = await async_playwright().start()
            browser = await pw_module.chromium.launch(
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
            from scanner.parsers.stealth import create_stealth_context, generate_stealth_config
            config = generate_stealth_config()
            context = await create_stealth_context(browser, config)
            page = await context.new_page()
            
            try:
                await page.goto(url, wait_until='domcontentloaded', timeout=60000)
                await asyncio.sleep(3)
                
                # Use the EXACT same JS as the Python parser
                events_data = await page.evaluate("""
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
                """)
                
                log(f"Found {len(events_data)} events via JS")
                result = []
                for e in events_data:
                    home = e.get('home', '')
                    away = e.get('away', '')
                    odds = e.get('odds', [])
                    if not home or not away or len(odds) < 2:
                        continue
                    if len(odds) == 2:
                        result.append({'home_team': home, 'away_team': away, 'league': '', 'home_odds': None, 'draw_odds': None, 'away_odds': None})
                    elif len(odds) >= 3:
                        result.append({'home_team': home, 'away_team': away, 'league': '', 'home_odds': odds[0], 'draw_odds': odds[1], 'away_odds': odds[2]})
                log(f"Returning {len(result)} events")
                print(json.dumps(result, ensure_ascii=False))
                sys.stdout.flush()
                return
            finally:
                await page.close()
                await context.close()
                await browser.close()
                await pw_module.stop()
        elif parser_name == 'zenit':
            from scanner.parsers.zenit_playwright import ZenitPlaywrightParser
            log("Imported ZenitPlaywrightParser")
            async with ZenitPlaywrightParser() as parser:
                parser.urls = [url]
                events = await parser.get_events()
                log(f"Got {len(events)} events")
        elif parser_name == 'betcity':
            from scanner.parsers.betcity_playwright import BetcityPlaywrightParser
            log("Imported BetcityPlaywrightParser")
            async with BetcityPlaywrightParser() as parser:
                parser.urls = [url]
                events = await parser.get_events()
                log(f"Got {len(events)} events")
        elif parser_name == 'baltbet':
            from scanner.parsers.baltbet_playwright import BaltbetPlaywrightParser
            log("Imported BaltbetPlaywrightParser")
            async with BaltbetPlaywrightParser() as parser:
                parser.urls = [url]
                events = await parser.get_events()
                log(f"Got {len(events)} events")
        else:
            log(f"Unknown parser: {parser_name}")
            events = []

        result = []
        for e in events:
            result.append({
                'home_team': e.get('home_team', ''),
                'away_team': e.get('away_team', ''),
                'league': e.get('league', ''),
                'home_odds': e.get('home_odds'),
                'draw_odds': e.get('draw_odds'),
                'away_odds': e.get('away_odds'),
                'is_live': e.get('is_live', False),
            })
        log(f"Returning {len(result)} events as JSON")
        print(json.dumps(result, ensure_ascii=False))
        sys.stdout.flush()
    except Exception as ex:
        log(f"Exception: {ex}")
        log(traceback.format_exc())
        print(json.dumps([]))
        sys.stdout.flush()

if __name__ == '__main__':
    log(f"Args: {sys.argv}")
    if len(sys.argv) < 3:
        print(json.dumps([]))
        sys.exit(0)
    asyncio.run(run_parser(sys.argv[1], sys.argv[2]))
