"""Winline Playwright scraper - extracts real events from DOM"""
import asyncio
import json
import sys
import os

async def scrape_winline():
    from playwright.async_api import async_playwright
    
    all_events = []
    seen = set()
    
    urls = [
        "https://winline.ru/football",
        "https://winline.ru/live/football",
        "https://winline.ru/basketball",
        "https://winline.ru/live/basketball",
        "https://winline.ru/hockey",
        "https://winline.ru/live/hockey",
        "https://winline.ru/tennis",
        "https://winline.ru/live/tennis",
        "https://winline.ru/volleyball",
        "https://winline.ru/live/volleyball",
        "https://winline.ru/table-tennis",
        "https://winline.ru/live/table-tennis",
        "https://winline.ru/baseball",
        "https://winline.ru/live/baseball",
        "https://winline.ru/handball",
        "https://winline.ru/live/handball",
        "https://winline.ru/cyber-sport",
        "https://winline.ru/live/cyber-sport",
    ]
    
    async with async_playwright() as p:
        browser = await p.chromium.launch(
            headless=True,
            args=[
                '--no-sandbox',
                '--disable-blink-features=AutomationControlled',
                '--disable-dev-shm-usage',
                '--disable-web-security',
            ]
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        
        # Stealth
        from playwright_stealth import Stealth
        Stealth().apply_stealth_sync(context)
        
        for url in urls:
            try:
                page = await context.new_page()
                is_live = 'live' in url
                
                # Загружаем страницу
                await page.goto(url, wait_until='domcontentloaded', timeout=20000)
                await page.wait_for_timeout(5000)
                
                # Скроллим для lazy load
                await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 3)")
                await page.wait_for_timeout(2000)
                await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
                await page.wait_for_timeout(2000)
                await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                await page.wait_for_timeout(2000)
                
                # Извлекаем события
                events = await page.evaluate('''
                    () => {
                        const events = [];
                        // Ищем все карточки событий
                        const selectors = [
                            'ww-feature-event-mini-card-dsk',
                            '[class*="event-card"]',
                            '[class*="event-item"]',
                            '[class*="match-card"]',
                            '.event',
                        ];
                        
                        let cards = [];
                        for (const sel of selectors) {
                            const found = document.querySelectorAll(sel);
                            if (found.length > 0) {
                                cards = Array.from(found);
                                break;
                            }
                        }
                        
                        // Если не нашли по селекторам, ищем по структуре
                        if (cards.length === 0) {
                            // Ищем элементы с кэфами
                            const allEls = document.querySelectorAll('*');
                            const eventContainers = new Set();
                            allEls.forEach(el => {
                                const text = el.textContent || '';
                                const odds = text.match(/\\d+[.,]\\d{1,2}/g);
                                if (odds && odds.length >= 3) {
                                    const parent = el.closest('[class*="event"], [class*="match"], .main-event');
                                    if (parent) eventContainers.add(parent);
                                }
                            });
                            cards = Array.from(eventContainers);
                        }
                        
                        cards.forEach(card => {
                            try {
                                let home = '', away = '';
                                
                                // Пробуем разные способы извлечения команд
                                const nameEls = card.querySelectorAll('.half__names .name, .team-name, .event__team');
                                if (nameEls.length >= 2) {
                                    home = (nameEls[0].getAttribute('title') || nameEls[0].textContent || '').trim();
                                    away = (nameEls[1].getAttribute('title') || nameEls[1].textContent || '').trim();
                                    if (home && home === away && home.includes(' - ')) {
                                        const parts = home.split(' - ');
                                        if (parts.length >= 2) { home = parts[0].trim(); away = parts[1].trim(); }
                                    }
                                }
                                
                                // Fallback: ищем любые названия
                                if (!home || !away) {
                                    const anyNames = card.querySelectorAll('.name, .team, .competitor');
                                    if (anyNames.length >= 2) {
                                        home = (anyNames[0].textContent || '').trim();
                                        away = (anyNames[1].textContent || '').trim();
                                    }
                                }
                                
                                // Ещё fallback: извлекаем из текста
                                if (!home || !away) {
                                    const lines = (card.textContent || '').split('\\n').map(l => l.trim()).filter(l => l.length > 2 && l.length < 50);
                                    const teams = lines.filter(l => !l.match(/^\\d+[.,]\\d+$/) && !l.match(/^LIVE/i) && l.length > 2);
                                    if (teams.length >= 2) {
                                        home = teams[0];
                                        away = teams[teams.length - 1];
                                    }
                                }
                                
                                if (!home || !away || home.length < 2) return;
                                
                                // Извлекаем кэфы
                                const odds = [];
                                const coefEls = card.querySelectorAll('.half__coef-buttons .button__coef-title, .coef, .odds, [class*="coef"]');
                                coefEls.forEach(btn => {
                                    const val = parseFloat(btn.textContent.trim().replace(',', '.'));
                                    if (!isNaN(val) && val >= 1.01 && val <= 100) odds.push(val);
                                });
                                
                                // Fallback для кэфов
                                if (odds.length < 2) {
                                    const textOdds = (card.textContent || '').match(/\\d+[.,]\\d{1,2}/g);
                                    if (textOdds) {
                                        textOdds.forEach(o => {
                                            const val = parseFloat(o.replace(',', '.'));
                                            if (!isNaN(val) && val >= 1.01 && val <= 100) odds.push(val);
                                        });
                                    }
                                }
                                
                                if (odds.length >= 2) {
                                    events.push({
                                        home,
                                        away,
                                        odds: odds.slice(0, 5),
                                        is_live: false // Определим позже
                                    });
                                }
                            } catch(e) {}
                        });
                        
                        return events;
                    }
                ''')
                
                for ev in events:
                    key = f"{ev['home']}|{ev['away']}"
                    if key not in seen:
                        seen.add(key)
                        ev['bookmaker'] = 'winline'
                        ev['league'] = ''
                        ev['is_live'] = is_live
                        all_events.append(ev)
                
                await page.close()
                print(f"  {url}: +{len(events)} raw events, total: {len(all_events)}", file=sys.stderr)
                
            except Exception as e:
                print(f"  {url}: Error: {e}", file=sys.stderr)
        
        await browser.close()
    
    print(f"Total: {len(all_events)} events", file=sys.stderr)
    return all_events

async def main():
    try:
        events = await scrape_winline()
        output = {
            "bookmaker": "winline",
            "events": events,
            "count": len(events),
        }
        # Save to file
        output_file = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'winline_events.json')
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump(output, f, ensure_ascii=False, default=str)
        print(f"Saved {len(events)} events to {output_file}", file=sys.stderr)
        # Output JSON to stdout
        print(json.dumps(output, ensure_ascii=False, default=str))
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc(file=sys.stderr)
        # Save empty
        print(json.dumps({"bookmaker": "winline", "events": [], "count": 0}))

if __name__ == "__main__":
    asyncio.run(main())
