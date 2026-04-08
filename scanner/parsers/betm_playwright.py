# scanner/parsers/betm_playwright.py
"""
Bet-M Playwright Parser
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class BetMPlaywrightParser:
    name = "Bet-M (Playwright)"
    slug = "betm"
    urls = [
        "https://bet-m.net/live",
        "https://bet-m.net/line",
    ]
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        for url in self.urls:
            max_retries = 2
            for attempt in range(max_retries):
                try:
                    events = await self._fetch_url(url)
                    all_events.extend(events)
                    break
                except Exception as e:
                    logger.warning(f"Bet-M attempt {attempt + 1}/{max_retries} failed for {url}: {e}")
                    if attempt < max_retries - 1:
                        await asyncio.sleep(5)
                    continue
        return all_events
    
    async def _fetch_url(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(headless=True)
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
        """)
        
        page = await context.new_page()
        await page.goto(url, wait_until='domcontentloaded', timeout=30000)
        await asyncio.sleep(10)
        
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
        await asyncio.sleep(2)
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await asyncio.sleep(3)
        
        events = await self._extract_events(page, url)
        logger.info(f"Bet-M ({url}): Extracted {len(events)} events")
        
        await browser.close()
        return events
    
    async def _extract_events(self, page, url: str) -> List[Dict]:
        raw_events = await page.evaluate("""
            () => {
                const events = [];
                const selectors = [
                    '.event-item', '.match-item', '.game-item',
                    '[class*="event"]', '[class*="match"]', '[class*="game"]',
                    '.sport-event', '.event-line'
                ];
                
                let containers = [];
                selectors.forEach(sel => {
                    containers.push(...document.querySelectorAll(sel));
                });
                containers = [...new Set(containers)];
                
                containers.forEach((el) => {
                    const text = el.textContent || '';
                    if (!text || text.length < 15) return;
                    
                    const odds = [];
                    el.querySelectorAll('[class*="coef"], [class*="kef"], .coef, .kef, span').forEach(n => {
                        const val = parseFloat(n.textContent.trim().replace(',', '.'));
                        if (!isNaN(val) && val >= 1.01 && val <= 50) {
                            odds.push(val);
                        }
                    });
                    
                    const lines = text.split(/\\n/).map(l => l.trim()).filter(l => l);
                    let home = '', away = '';
                    
                    for (const line of lines) {
                        if (line.length > 2 && line.length < 50 &&
                            !line.match(/^\\d+[.,]\\d+$/) &&
                            !line.match(/LIVE/i)) {
                            if (!home) home = line;
                            else if (line !== home && !away) away = line;
                        }
                        if (home && away) break;
                    }
                    
                    if (home && away && odds.length >= 2) {
                        events.push({ home, away, odds: odds.slice(0, 3) });
                    }
                });
                
                return events;
            }
        """)
        
        return self._normalize_events(raw_events, url)
    
    def _normalize_events(self, raw_events: list, url: str) -> List[Dict]:
        result = []
        seen = set()
        
        for i, e in enumerate(raw_events):
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            
            if not home or not away or len(home) < 2 or len(away) < 2:
                continue
            
            key = f"{home}|{away}"
            if key in seen:
                continue
            seen.add(key)
            
            if len(odds) < 2:
                continue
            
            is_3way = len(odds) >= 3
            
            event = {
                'id': f"betm_{i}_{hash(key) % 1000000}",
                'bookmaker': 'betm',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': 'Live' if 'live' in url else 'Pre-match',
                'home_odds': odds[0],
                'draw_odds': odds[1] if is_3way else None,
                'away_odds': odds[2] if is_3way else odds[1],
                'is_live': 'live' in url,
                'market': '1x2',
                'source_url': url,
                'scraped_at': time.time()
            }
            
            if event['home_odds'] >= 1.01:
                result.append(event)
        
        return result


async def test():
    logging.basicConfig(level=logging.INFO)
    parser = BetMPlaywrightParser()
    events = await parser.get_events()
    print(f'Bet-M: {len(events)} events')

if __name__ == '__main__':
    asyncio.run(test())
