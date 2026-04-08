# scanner/parsers/marathon_playwright.py
"""
Marathon Playwright Parser - Simple text extraction
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class MarathonPlaywrightParser:
    """Marathon parser - simple text extraction."""
    
    name = "Marathon (Playwright)"
    slug = "marathon"
    urls = [
        "https://www.marathonbet.ru/su/live",
        "https://www.marathonbet.ru/su/line",
        "https://www.marathonbet.ru/su/live/basketball",
        "https://www.marathonbet.ru/su/line/basketball",
        "https://www.marathonbet.ru/su/live/hockey",
        "https://www.marathonbet.ru/su/line/hockey",
        "https://www.marathonbet.ru/su/live/tennis",
        "https://www.marathonbet.ru/su/line/tennis",
        "https://www.marathonbet.ru/su/live/volleyball",
        "https://www.marathonbet.ru/su/line/volleyball",
        "https://www.marathonbet.ru/su/live/baseball",
        "https://www.marathonbet.ru/su/line/baseball",
        "https://www.marathonbet.ru/su/live/handball",
        "https://www.marathonbet.ru/su/line/handball",
        "https://www.marathonbet.ru/su/live/rugby",
        "https://www.marathonbet.ru/su/line/rugby",
        "https://www.marathonbet.ru/su/live/table-tennis",
        "https://www.marathonbet.ru/su/line/table-tennis",
        "https://www.marathonbet.ru/su/live/badminton",
        "https://www.marathonbet.ru/su/line/badminton",
    ]
    
    def __init__(self):
        self.browser = None
        self.context = None
        self.page = None
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        try:
            pw = await async_playwright().start()
            self.browser = await pw.chromium.launch(
                headless=True,
                args=['--disable-blink-features=AutomationControlled']
            )
            self.context = await self.browser.new_context(
                user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                viewport={'width': 1920, 'height': 1080},
                locale='ru-RU',
            )
            await self.context.add_init_script("""
                Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
                Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
                window.chrome = { runtime: {} };
            """)
            
            for url_idx, url in enumerate(self.urls):
                try:
                    self.page = await self.context.new_page()
                    
                    # Block heavy resources
                    await self.page.route('**/*.{png,jpg,jpeg,gif,svg,webp}', lambda route: route.abort())
                    await self.page.route('**/analytics/**', lambda route: route.abort())
                    
                    await self.page.goto(url, wait_until='domcontentloaded', timeout=15000)
                    await asyncio.sleep(3)
                    
                    # Scroll
                    for i in range(3):
                        await self.page.evaluate("window.scrollTo(0, document.body.scrollHeight / 3 * {})".format(i+1))
                        await asyncio.sleep(0.5)
                    
                    events = await self._extract_events(url)
                    all_events.extend(events)
                    logger.info(f"Marathon ({url}): {len(events)} events")
                    
                    if self.page:
                        await self.page.close()
                    
                    if len(all_events) > 0 and url_idx >= 3:
                        logger.info(f"Marathon: early break after {url_idx + 1} URLs, {len(all_events)} events")
                        break
                    
                except Exception as e:
                    logger.warning(f"Marathon failed for {url}: {e}")
                    if self.page:
                        await self.page.close()
            
            await self.browser.close()
        except Exception as e:
            logger.error(f"Marathon error: {e}")
        
        return all_events
    
    async def _extract_events(self, url: str) -> List[Dict]:
        """Extract events using simple text parsing."""
        try:
            raw_events = await self.page.evaluate("""
                () => {
                    const events = [];
                    const containers = document.querySelectorAll('[class*="event"], [class*="match"], [class*="game"], .sport-event, .event-line, [class*="coupon"]');
                    
                    containers.forEach(el => {
                        const text = el.textContent || '';
                        if (!text || text.length < 20) return;
                        
                        const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                        const teams = [];
                        const odds = [];
                        
                        for (const line of lines) {
                            const val = parseFloat(line.replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 50) {
                                odds.push(val);
                            } else if (line.length > 2 && line.length < 40 && !line.match(/LIVE|live/i)) {
                                teams.push(line);
                            }
                            if (teams.length >= 2 && odds.length >= 1) break;
                        }
                        
                        if (teams.length >= 2 && odds.length >= 1) {
                            events.push({
                                home_team: teams[0],
                                away_team: teams[1],
                                home_odds: odds[0] || 0,
                                draw_odds: odds.length >= 3 ? odds[1] : null,
                                away_odds: odds.length >= 3 ? odds[2] : (odds[1] || 0)
                            });
                        }
                    });
                    
                    return events;
                }
            """)
            
            result = []
            for raw in raw_events:
                if raw.get('home_team') and raw.get('away_team'):
                    result.append({
                        'id': f"marathon_{hash(raw['home_team'] + raw['away_team'])}",
                        'bookmaker': 'marathon',
                        'sport': 'football',
                        'home_team': raw['home_team'],
                        'away_team': raw['away_team'],
                        'league': 'Live' if 'live' in url else 'Pre-match',
                        'home_odds': raw.get('home_odds', 0),
                        'draw_odds': raw.get('draw_odds'),
                        'away_odds': raw.get('away_odds', 0),
                        'is_live': 'live' in url,
                        'market': '1x2',
                        'source_url': url,
                        'scraped_at': time.time()
                    })
            
            return result
        except Exception as e:
            logger.warning(f"Marathon extraction error: {e}")
            return []
