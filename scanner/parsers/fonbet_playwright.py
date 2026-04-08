# scanner/parsers/fonbet_playwright.py
"""
Fonbet Playwright Parser - Text extraction from main page
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class FonbetPlaywrightParser:
    name = "Fonbet (Playwright)"
    slug = "fonbet"
    urls = [
        "https://fonbet.ru/live/football",
        "https://fonbet.ru/line/football",
        "https://fonbet.ru/live/basketball",
        "https://fonbet.ru/line/basketball",
        "https://fonbet.ru/live/hockey",
        "https://fonbet.ru/line/hockey",
        "https://fonbet.ru/live/tennis",
        "https://fonbet.ru/line/tennis",
        "https://fonbet.ru/live/volleyball",
        "https://fonbet.ru/line/volleyball",
        "https://fonbet.ru/live/baseball",
        "https://fonbet.ru/line/baseball",
        "https://fonbet.ru/live/handball",
        "https://fonbet.ru/line/handball",
        "https://fonbet.ru/live/rugby",
        "https://fonbet.ru/line/rugby",
        "https://fonbet.ru/live/table-tennis",
        "https://fonbet.ru/line/table-tennis",
        "https://fonbet.ru/live/badminton",
        "https://fonbet.ru/line/badminton",
    ]
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        for url in self.urls:
            try:
                events = await self._fetch_url(url)
                all_events.extend(events)
                if events:
                    break
            except Exception as e:
                logger.warning(f"Fonbet failed for {url}: {e}")
        return all_events
    
    async def _fetch_url(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(
            headless=True,
            args=['--disable-blink-features=AutomationControlled']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            window.chrome = {runtime: {}};
        """)
        
        page = await context.new_page()
        
        try:
            await page.goto(url, wait_until='domcontentloaded', timeout=30000)
            await asyncio.sleep(12)
            
            # Scroll to load more content
            await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 3)")
            await asyncio.sleep(2)
            await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
            await asyncio.sleep(2)
            await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
            await asyncio.sleep(2)
            
            events = await self._extract_events(page, url)
            logger.info(f"Fonbet ({url}): {len(events)} events")
        except Exception as e:
            logger.warning(f"Fonbet error: {e}")
            events = []
        finally:
            await browser.close()
        
        return events
    
    async def _extract_events(self, page, url: str) -> List[Dict]:
        """Extract events from page text."""
        try:
            raw_events = await page.evaluate("""
                () => {
                    const events = [];
                    const text = document.body.innerText;
                    const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                    
                    let i = 0;
                    while (i < lines.length - 2) {
                        const line = lines[i];
                        
                        // Look for team names (not numbers, not LIVE, not scores)
                        if (line.length > 3 && line.length < 50 && 
                            !line.match(/^\\d/) && !line.match(/LIVE|live/i) &&
                            !line.match(/^\\d+[.,]\\d+$/) &&
                            !line.match(/^\\d+:\\d/)) {
                            
                            const odds = [];
                            const teamLines = [line];
                            let j = i + 1;
                            
                            while (j < lines.length && odds.length < 8) {
                                const val = parseFloat(lines[j].replace(',', '.'));
                                if (!isNaN(val) && val >= 1.01 && val <= 50) {
                                    odds.push(val);
                                } else if (lines[j].length > 3 && lines[j].length < 50 && 
                                           !lines[j].match(/^\\d/) && teamLines.length < 2) {
                                    teamLines.push(lines[j]);
                                }
                                j++;
                                if (odds.length >= 2 && teamLines.length >= 2) break;
                            }
                            
                            if (teamLines.length >= 2 && odds.length >= 1) {
                                events.push({
                                    home: teamLines[0],
                                    away: teamLines[teamLines.length - 1],
                                    odds: odds.slice(0, 5)
                                });
                                i = j;
                                continue;
                            }
                        }
                        i++;
                    }
                    
                    return events;
                }
            """)
            
            return self._normalize(raw_events, url)
        except Exception as e:
            logger.warning(f"Fonbet extraction error: {e}")
            return []
    
    def _normalize(self, raw_events: list, url: str) -> List[Dict]:
        result = []
        seen = set()
        
        for e in raw_events:
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            
            if not home or not away or len(home) < 2 or len(away) < 2:
                continue
            
            key = f"{home}|{away}"
            if key in seen:
                continue
            seen.add(key)
            
            if len(odds) < 1:
                continue
            
            result.append({
                'id': f"fonbet_{hash(key) % 1000000}",
                'bookmaker': 'fonbet',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': 'Live' if 'live' in url else 'Pre-match',
                'home_odds': odds[0],
                'draw_odds': odds[1] if len(odds) > 2 else None,
                'away_odds': odds[2] if len(odds) > 2 else (odds[1] if len(odds) > 1 else 0),
                'is_live': 'live' in url,
                'market': '1x2',
                'source_url': url,
                'scraped_at': time.time()
            })
        
        return result
