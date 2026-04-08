# scanner/parsers/betcity_playwright.py
"""
Betcity Playwright Parser - Simple text extraction
"""
import asyncio
import time
import re
from typing import List, Dict, Optional
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class BetcityPlaywrightParser:
    name = "Betcity (Playwright)"
    slug = "betcity"
    urls = [
        "https://betcity.ru/ru/live",
        "https://betcity.ru/ru/line/football",
        "https://betcity.ru/ru/live/cyber-sport",
        "https://betcity.ru/ru/live/basketball",
        "https://betcity.ru/ru/line/basketball",
        "https://betcity.ru/ru/live/hockey",
        "https://betcity.ru/ru/line/hockey",
        "https://betcity.ru/ru/live/tennis",
        "https://betcity.ru/ru/line/tennis",
        "https://betcity.ru/ru/live/volleyball",
        "https://betcity.ru/ru/line/volleyball",
        "https://betcity.ru/ru/live/baseball",
        "https://betcity.ru/ru/line/baseball",
        "https://betcity.ru/ru/live/handball",
        "https://betcity.ru/ru/line/handball",
        "https://betcity.ru/ru/live/rugby",
        "https://betcity.ru/ru/line/rugby",
        "https://betcity.ru/ru/live/table-tennis",
        "https://betcity.ru/ru/line/table-tennis",
        "https://betcity.ru/ru/live/badminton",
        "https://betcity.ru/ru/line/badminton",
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
                    await self.page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
                    await asyncio.sleep(1)
                    await self.page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                    await asyncio.sleep(1)
                    
                    events = await self._extract_events(url)
                    all_events.extend(events)
                    logger.info(f"Betcity ({url}): {len(events)} events")
                    
                    if self.page:
                        await self.page.close()
                    
                    if len(all_events) > 0 and url_idx >= 3:
                        logger.info(f"Betcity: early break after {url_idx + 1} URLs, {len(all_events)} events")
                        break
                    
                except Exception as e:
                    logger.warning(f"Betcity failed for {url}: {e}")
                    if self.page:
                        await self.page.close()
            
            await self.browser.close()
        except Exception as e:
            logger.error(f"Betcity error: {e}")
        
        return all_events
    
    async def _extract_events(self, url: str) -> List[Dict]:
        """Extract events using Betcity's specific selectors."""
        try:
            raw_events = await self.page.evaluate("""
                () => {
                    const events = [];
                    const containers = document.querySelectorAll('.line-event');
                    
                    containers.forEach(el => {
                        try {
                            // Get team names from .line-event__name-text
                            const nameTexts = el.querySelectorAll('.line-event__name-text');
                            const teams = [];
                            nameTexts.forEach(nt => {
                                const t = nt.textContent.trim();
                                if (t) teams.push(t);
                            });
                            
                            // Get odds from .line-event__main-bets-button
                            const oddsButtons = el.querySelectorAll('.line-event__main-bets-button');
                            const odds = [];
                            oddsButtons.forEach(btn => {
                                const t = btn.textContent.trim();
                                const val = parseFloat(t.replace(',', '.'));
                                if (!isNaN(val) && val >= 1.01 && val <= 100) {
                                    odds.push(val);
                                }
                            });
                            
                            if (teams.length >= 2 && odds.length >= 2) {
                                events.push({
                                    home_team: teams[0],
                                    away_team: teams[1],
                                    home_odds: odds[0] || 0,
                                    draw_odds: odds.length >= 3 ? odds[1] : null,
                                    away_odds: odds.length >= 3 ? odds[2] : (odds[1] || 0)
                                });
                            }
                        } catch(e) {}
                    });
                    
                    return events;
                }
            """)
            
            result = []
            for raw in raw_events:
                if raw.get('home_team') and raw.get('away_team'):
                    result.append({
                        'id': f"betcity_{hash(raw['home_team'] + raw['away_team'])}",
                        'bookmaker': 'betcity',
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
            logger.warning(f"Betcity extraction error: {e}")
            return []
