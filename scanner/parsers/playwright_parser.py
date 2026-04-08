# scanner/parsers/playwright_parser.py
"""
Playwright-based parser for extracting real data from SPAs
"""
import asyncio
import json
import time
import logging
from typing import List, Dict, Optional
from playwright.async_api import async_playwright, Page

logger = logging.getLogger(__name__)


class PlaywrightParser:
    """
    Base class for Playwright-based parsers.
    Renders JavaScript and extracts real odds data.
    """
    
    name = "Playwright"
    slug = "playwright"
    base_url = ""
    selectors = {
        'event_container': '',
        'odds_button': '',
        'team_home': '',
        'team_away': '',
    }
    
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
            ]
        )
        self.context = await self.browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await self.context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
            window.chrome = { runtime: {} };
        """)
        return self
    
    async def __aexit__(self, *args):
        if self.page:
            await self.page.close()
        if self.context:
            await self.context.close()
        if self.browser:
            await self.browser.close()
    
    async def get_page(self) -> Page:
        if not self.page:
            self.page = await self.context.new_page()
        return self.page
    
    async def get_events(self) -> List[Dict]:
        """Get events using Playwright"""
        try:
            page = await self.get_page()
            
            # Navigate to page
            await page.goto(self.base_url, wait_until="domcontentloaded", timeout=30000)
            
            # Wait for dynamic content
            await asyncio.sleep(5)
            
            # Try to extract data from page
            events = await self._extract_events(page)
            
            # If no events, try to get from window objects
            if not events:
                events = await self._extract_from_window(page)
            
            return events
            
        except Exception as e:
            logger.error(f"{self.slug} Playwright error: {e}")
            return []
    
    async def _extract_events(self, page: Page) -> List[Dict]:
        """Extract events from page elements"""
        events = []
        
        # Try to find event containers
        containers = await page.query_selector_all('[class*="event"], [class*="match"], [class*="sport"]')
        
        for container in containers[:50]:
            try:
                event = await self._extract_event_from_container(container)
                if event:
                    events.append(event)
            except:
                continue
        
        return events
    
    async def _extract_from_window(self, page: Page) -> List[Dict]:
        """Extract events from window objects"""
        events = []
        
        try:
            # Try to get data from window objects
            data = await page.evaluate("""
                () => {
                    const data = {};
                    
                    // Try common variable names
                    const keys = ['__INITIAL_STATE__', '__PRELOADED_STATE__', '__DATA__', 
                                  'initialState', 'store', 'events', 'lineData'];
                    
                    for (const key of keys) {
                        if (window[key]) {
                            data[key] = window[key];
                        }
                    }
                    
                    // Try to find in scripts
                    const scripts = document.querySelectorAll('script[type="application/json"], script[id*="__NEXT_DATA__"]');
                    scripts.forEach((s, i) => {
                        try {
                            const parsed = JSON.parse(s.textContent);
                            data['script_' + i] = parsed;
                        } catch(e) {}
                    });
                    
                    return data;
                }
            """)
            
            if data:
                events = self._parse_window_data(data)
            
        except Exception as e:
            logger.debug(f"{self.slug} window extraction error: {e}")
        
        return events
    
    def _parse_window_data(self, data: Dict) -> List[Dict]:
        """Parse events from window data"""
        events = []
        
        def extract_items(obj):
            if isinstance(obj, dict):
                if 'home' in obj and 'away' in obj and ('odds' in obj or 'k1' in obj):
                    event = self._create_event(obj)
                    if event:
                        events.append(event)
                
                for v in obj.values():
                    extract_items(v)
                    
            elif isinstance(obj, list):
                for item in obj:
                    extract_items(item)
        
        extract_items(data)
        return events
    
    def _extract_event_from_container(self, container) -> Optional[Dict]:
        """Extract event from container element"""
        return None  # Override in subclasses
    
    def _create_event(self, data: Dict) -> Dict:
        """Create event dict from raw data"""
        try:
            home = data.get('home') or data.get('homeTeam', '')
            away = data.get('away') or data.get('awayTeam', '')
            
            if not home or not away:
                return None
            
            odds = data.get('odds', {})
            if isinstance(odds, dict):
                home_odds = float(odds.get('1') or odds.get('w1') or odds.get('home', 0))
                away_odds = float(odds.get('2') or odds.get('w2') or odds.get('away', 0))
                draw_odds = float(odds.get('X') or odds.get('draw') or 0)
            else:
                home_odds = float(data.get('k1') or data.get('homeOdds') or 0)
                away_odds = float(data.get('k2') or data.get('awayOdds') or 0)
                draw_odds = float(data.get('kx') or data.get('drawOdds') or 0)
            
            if home_odds < 1.01 and away_odds < 1.01:
                return None
            
            return {
                'id': f"{self.slug}_{data.get('id', hash(str(home) + str(away)))}",
                'bookmaker': self.slug,
                'sport': data.get('sport', 'football'),
                'home_team': str(home),
                'away_team': str(away),
                'league': data.get('league') or data.get('champ') or data.get('tournament', 'Live'),
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': True,
                'market': '1x2',
                'source_url': self.base_url,
                'scraped_at': time.time()
            }
            
        except:
            return None


class WinlinePlaywrightParser(PlaywrightParser):
    """Playwright parser for Winline"""
    name = "Winline (PW)"
    slug = "winline"
    base_url = "https://winline.ru/live/football"
    
    selectors = {
        'event_container': '[class*="sport-category-event"], [class*="event-card"]',
        'team_home': '[class*="team-name"]:first-child, [class*="home"]',
        'team_away': '[class*="team-name"]:last-child, [class*="away"]',
    }


class FonbetPlaywrightParser(PlaywrightParser):
    """Playwright parser for Fonbet"""
    name = "Fonbet (PW)"
    slug = "fonbet"
    base_url = "https://www.fonbet.ru/live/football/"


class BetBoomPlaywrightParser(PlaywrightParser):
    """Playwright parser for BetBoom"""
    name = "BetBoom (PW)"
    slug = "betboom"
    base_url = "https://betboom.ru/live"


class PariPlaywrightParser(PlaywrightParser):
    """Playwright parser for Pari"""
    name = "Pari (PW)"
    slug = "pari"
    base_url = "https://pari.ru/live"


async def test_parsers():
    """Test all Playwright parsers"""
    parsers = [
        WinlinePlaywrightParser,
        FonbetPlaywrightParser,
        BetBoomPlaywrightParser,
        PariPlaywrightParser,
    ]
    
    results = []
    
    for parser_class in parsers:
        try:
            async with parser_class() as parser:
                events = await parser.get_events()
                results.append({
                    'name': parser_class.name,
                    'count': len(events),
                    'status': 'OK' if events else 'Empty'
                })
                logger.info(f"{parser_class.name}: {len(events)} events")
        except Exception as e:
            results.append({
                'name': parser_class.name,
                'count': 0,
                'status': f'Error: {e}'
            })
            logger.error(f"{parser_class.name}: {e}")
    
    return results


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    results = asyncio.run(test_parsers())
    for r in results:
        print(f"{r['name']}: {r['count']} events - {r['status']}")
