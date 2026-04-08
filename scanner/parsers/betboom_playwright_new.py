# scanner/parsers/winline_playwright.py
"""
Winline Playwright parser - extracts real odds from the website
"""
import asyncio
import time
from typing import List, Dict, Optional
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class WinlinePlaywrightParser:
    """Winline parser using Playwright for SPA rendering."""
    
    name = "Winline (Playwright)"
    slug = "winline"
    base_url = "https://winline.ru/live/football"
    
    def __init__(self):
        self.browser = None
        self.context = None
        self.page = None
    
    async def __aenter__(self):
        pw = await async_playwright().start()
        self.browser = await pw.chromium.launch(headless=True)
        self.context = await self.browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await self.context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
        """)
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
        try:
            if self.context is None:
                pw = await async_playwright().start()
                self.browser = await pw.chromium.launch(headless=True)
                self.context = await self.browser.new_context(
                    user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                    viewport={'width': 1920, 'height': 1080},
                    locale='ru-RU',
                )
                await self.context.add_init_script("""
                    Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
                """)
            
            self.page = await self.context.new_page()
            
            await self.page.goto(self.base_url, wait_until='domcontentloaded', timeout=30000)
            await asyncio.sleep(5)
            
            events = await self._extract_events()
            
            logger.info(f"Winline: Extracted {len(events)} events")
            return events
            
        except Exception as e:
            logger.error(f"Winline Playwright error: {e}")
            return []
    
    async def _extract_events(self) -> List[Dict]:
        """Extract events with odds from page"""
        
        events_data = await self.page.evaluate("""
            () => {
                const events = [];
                
                const coefContainers = document.querySelectorAll('.half__coef-buttons');
                
                coefContainers.forEach(container => {
                    try {
                        const eventContainer = container.closest('[class*="event"], [class*="match"], [class*="item"]');
                        
                        let home = '';
                        let away = '';
                        let tournament = '';
                        
                        if (eventContainer) {
                            const titleEl = eventContainer.querySelector('.main-event__title, [class*="team"], [class*="name"]');
                            if (titleEl) {
                                const text = titleEl.textContent.trim();
                                const parts = text.split(/\\s{2,}|vs|против|VS|-/).map(s => s.trim()).filter(s => s);
                                if (parts.length >= 2) {
                                    home = parts[0];
                                    away = parts[1];
                                } else if (text.length > 3) {
                                    home = text;
                                }
                            }
                            
                            const tourEl = eventContainer.querySelector('.main-event__tournament, [class*="tournament"], [class*="league"]');
                            if (tourEl) {
                                tournament = tourEl.textContent.trim();
                            }
                        }
                        
                        const odds = [];
                        const coefButtons = container.querySelectorAll('.coef-buttons__button:not(.coef-buttons__button_locked)');
                        coefButtons.forEach(btn => {
                            const titleEl = btn.querySelector('.button__coef-title');
                            if (titleEl) {
                                const text = titleEl.textContent.trim();
                                const val = parseFloat(text.replace(',', '.'));
                                if (!isNaN(val) && val >= 1.01) {
                                    odds.push(val);
                                }
                            }
                        });
                        
                        if (odds.length >= 2) {
                            events.push({home, away, tournament, odds});
                        }
                    } catch(e) {}
                });
                
                return events;
            }
        """)
        
        result = []
        for i, e in enumerate(events_data):
            home = e.get('home', '')
            away = e.get('away', '')
            odds = e.get('odds', [])
            
            if not home or not away:
                # Try to split single team name
                parts = home.split()
                if len(parts) >= 2:
                    home = ' '.join(parts[:len(parts)//2])
                    away = ' '.join(parts[len(parts)//2:])
                else:
                    continue
            
            if len(odds) == 2:
                event = {
                    'id': f"winline_{i}_{hash(home + away)}",
                    'bookmaker': 'winline',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': e.get('tournament', 'Live'),
                    'home_odds': odds[0],
                    'draw_odds': None,
                    'away_odds': odds[1],
                    'is_live': True,
                    'market': '1x2',
                    'source_url': self.base_url,
                    'scraped_at': time.time()
                }
            elif len(odds) >= 3:
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
                    'source_url': self.base_url,
                    'scraped_at': time.time()
                }
            else:
                continue
            
            if event['home_odds'] >= 1.01:
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
