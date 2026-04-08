# scanner/parsers/winline_playwright.py
"""
Winline Playwright parser - extracts real odds from the website
"""
import asyncio
import time
from typing import List, Dict, Optional
import logging
from playwright.async_api import async_playwright
from scanner.parsers.stealth import create_stealth_context, generate_stealth_config

logger = logging.getLogger(__name__)


class WinlinePlaywrightParser:
    """Winline parser using Playwright for SPA rendering."""
    
    name = "Winline (Playwright)"
    slug = "winline"
    urls = [
        "https://winline.ru/live/football",
        "https://winline.ru/football",
        "https://winline.ru/live/basketball",
        "https://winline.ru/basketball",
        "https://winline.ru/live/hockey",
        "https://winline.ru/hockey",
        "https://winline.ru/live/tennis",
        "https://winline.ru/tennis",
        "https://winline.ru/live/volleyball",
        "https://winline.ru/volleyball",
        "https://winline.ru/live/cyber-football",
        "https://winline.ru/cyber-football",
        "https://winline.ru/live/cyber-sport",
        "https://winline.ru/cyber-sport",
        "https://winline.ru/live/table-tennis",
        "https://winline.ru/table-tennis",
        "https://winline.ru/live/baseball",
        "https://winline.ru/baseball",
        "https://winline.ru/live/handball",
        "https://winline.ru/handball",
        "https://winline.ru/live/rugby",
        "https://winline.ru/rugby",
        "https://winline.ru/live/badminton",
        "https://winline.ru/badminton",
    ]
    
    def __init__(self):
        self.browser = None
        self.context = None
        self.page = None
    
    async def __aenter__(self):
        pw = await async_playwright().start()
        self.browser = await pw.chromium.launch(
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
        self.context = await create_stealth_context(self.browser, config)
        return self
    
    async def __aexit__(self, *args):
        if self.page:
            await self.page.close()
        if self.context:
            await self.context.close()
        if self.browser:
            await self.browser.close()
    
    async def get_events(self) -> List[Dict]:
        """Get events using Playwright"""
        all_events = []
        for url in self.urls:
            try:
                if self.context is None:
                    pw = await async_playwright().start()
                    self.browser = await pw.chromium.launch(
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
                    self.context = await create_stealth_context(self.browser, config)
                
                self.page = await self.context.new_page()
                
                await self.page.goto(url, wait_until='domcontentloaded', timeout=60000)
                await asyncio.sleep(3)
                
                events = await self._extract_events(url)
                for e in events:
                    e['is_live'] = 'live' in url
                    e['league'] = 'Live' if 'live' in url else 'Pre-match'
                all_events.extend(events)
                
            except Exception as e:
                logger.error(f"Winline Playwright error for {url}: {e}")
        
        logger.info(f"Winline: Extracted {len(all_events)} events total")
        return all_events
    
    async def _extract_events(self, url: Optional[str] = None) -> List[Dict]:
        """Extract events with odds from page"""
        source_url = url or self.urls[0]
        
        events_data = await self.page.evaluate("""
            () => {
                const events = [];
                const isValidName = (t) => {
                    if (!t || t.length < 2 || t.length > 80) return false;
                    if (t === '-' || /^[-\\s]+$/.test(t)) return false;
                    if (/^(event|match|game|live|pre)/i.test(t)) return false;
                    return true;
                };

                // Each ww-feature-event-mini-card-dsk is one event
                const cards = document.querySelectorAll('ww-feature-event-mini-card-dsk');

                cards.forEach(card => {
                    try {
                        let home = '';
                        let away = '';
                        let tournament = '';

                        // Get team names from .half__names .name
                        const nameEls = card.querySelectorAll('.half__names .name');
                        if (nameEls.length >= 2) {
                            home = (nameEls[0].getAttribute('title') || nameEls[0].textContent || '').trim();
                            away = (nameEls[1].getAttribute('title') || nameEls[1].textContent || '').trim();
                            // title may contain "Home - Away", split if both names are combined
                            if (home && home === away && home.includes(' - ')) {
                                const parts = home.split(' - ');
                                if (parts.length >= 2) { home = parts[0].trim(); away = parts[1].trim(); }
                            }
                        }

                        // Fallback: any .name elements
                        if (!home || !away) {
                            const anyNames = card.querySelectorAll('.name');
                            if (anyNames.length >= 2) {
                                home = (anyNames[0].getAttribute('title') || anyNames[0].textContent || '').trim();
                                away = (anyNames[1].getAttribute('title') || anyNames[1].textContent || '').trim();
                            }
                        }

                        if (!isValidName(home) || !isValidName(away)) return;

                        // Get event ID from link
                        const link = card.querySelector('a[href*="/stavki/event/"]');
                        if (link) {
                            const href = link.getAttribute('href') || '';
                            const m = href.match(/\\/stavki\\/event\\/(\\d+)/);
                            if (m) tournament = 'event_' + m[1];
                        }

                        // Extract only the first 3 odds (1x2) from this card's own buttons
                        const odds = [];
                        const coefBtns = card.querySelectorAll('.half__coef-buttons .button__coef-title');
                        coefBtns.forEach(btn => {
                            const text = btn.textContent.trim();
                            const val = parseFloat(text.replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 100) {
                                odds.push(val);
                            }
                        });

                        if (odds.length >= 3) {
                            events.push({ home, away, tournament, odds: odds.slice(0, 3) });
                        } else if (odds.length === 2) {
                            events.push({ home, away, tournament, odds });
                        }
                    } catch(e) {}
                });

                // Also handle main-event block if present
                const mainEl = document.querySelector('.main-event');
                if (mainEl) {
                    const titleEl = mainEl.querySelector('.main-event__title');
                    const tourEl = mainEl.querySelector('.main-event__tournament');
                    if (titleEl) {
                        const raw = titleEl.textContent.trim();
                        const parts = raw.split(' - ');
                        if (parts.length >= 2 && isValidName(parts[0]) && isValidName(parts[1])) {
                            const mainOdds = [];
                            mainEl.querySelectorAll('.button__coef-title, .main-event__coeff').forEach(btn => {
                                const val = parseFloat(btn.textContent.trim().replace(',', '.'));
                                if (!isNaN(val) && val >= 1.01 && val <= 100) mainOdds.push(val);
                            });
                            events.push({
                                home: parts[0].trim(),
                                away: parts[1].trim(),
                                tournament: tourEl ? tourEl.textContent.trim() : '',
                                odds: mainOdds.slice(0, 3)
                            });
                        }
                    }
                }

                return events;
            }
        """)
        
        result = []
        for i, e in enumerate(events_data):
            home = e.get('home', '')
            away = e.get('away', '')
            odds = e.get('odds', [])
            
            if not home or not away:
                # Skip events without proper team names
                continue
            
            total_line = e.get('totalLine')
            total_lines = e.get('totalLines') or []
            
            if len(odds) == 2:
                # 2-way events are totals markets (over/under)
                total_over = {}
                total_under = {}
                
                if total_line:
                    # Current displayed line
                    total_over[total_line] = odds[0]
                    total_under[total_line] = odds[1]
                
                # Add all available lines with same odds (UI shows one line at a time)
                for line in total_lines:
                    if line not in total_over:
                        total_over[line] = odds[0]
                        total_under[line] = odds[1]
                
                event = {
                    'id': f"winline_{i}_{hash(home + away)}",
                    'bookmaker': 'winline',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': e.get('tournament', 'Live'),
                    'home_odds': None,
                    'draw_odds': None,
                    'away_odds': None,
                    'is_live': True,
                    'market': 'total',
                    'total_over': total_over,
                    'total_under': total_under,
                    'total_line': total_line,
                    'total_lines': total_lines,
                    'source_url': source_url,
                    'scraped_at': time.time()
                }
            elif len(odds) >= 3:
                # 3-way events are 1x2 markets (home/draw/away)
                event = {
                    'id': f"winline_{i}_{hash(home + away)}",
                    'bookmaker': 'winline',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': e.get('tournament', 'Live'),
                    'home_odds': odds[0],
                    'draw_odds': odds[1],
                    'away_odds': odds[2],
                    'is_live': True,
                    'market': '1x2',
                    'total_over': {},
                    'total_under': {},
                    'total_line': None,
                    'total_lines': [],
                    'source_url': source_url,
                    'scraped_at': time.time()
                }
            else:
                continue
            
            if event.get('home_odds') is not None and event['home_odds'] >= 1.01:
                result.append(event)
            elif event.get('home_odds') is None and event.get('total_over'):
                result.append(event)
        
        return result


async def test_parser():
    """Test the parser"""
    logging.basicConfig(level=logging.INFO)
    
    async with WinlinePlaywrightParser() as parser:
        events = await parser.get_events()
        print(f'\nFound {len(events)} events')
        
        two_way = [e for e in events if e['draw_odds'] is None]
        three_way = [e for e in events if e['draw_odds'] is not None]
        
        print(f'2-way events: {len(two_way)}')
        print(f'3-way events: {len(three_way)}')
        
        for i, e in enumerate(events[:3]):
            draw = f" | {e.get('draw_odds')}" if e.get('draw_odds') else ""
            print(f"\n{i+1}. {e['home_team']} vs {e['away_team']}")
            print(f"   Odds: {e['home_odds']}{draw} | {e['away_odds']}")


if __name__ == '__main__':
    asyncio.run(test_parser())
