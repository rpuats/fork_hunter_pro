# scanner/parsers/olimp_playwright.py
"""
Olimp Playwright Parser - Real data extraction from SPA
NOTE: Olimp uses heavy SPA/React that renders data client-side.
This parser attempts DOM extraction but may get 0 events due to site protection.
"""
import asyncio
import time
import re
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class OlimpPlaywrightParser:
    name = "Olimp (Playwright)"
    slug = "olimp"
    urls = [
        "https://www.olimp.bet/live",
        "https://www.olimp.bet/line",
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
                    logger.warning(f"Olimp attempt {attempt + 1}/{max_retries} failed for {url}: {e}")
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
            extra_http_headers={
                'Accept-Language': 'ru-RU,ru;q=0.9',
            }
        )
        await context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
            Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
            Object.defineProperty(navigator, 'languages', { get: () => ['ru-RU', 'ru', 'en-US', 'en'] });
        """)
        
        page = await context.new_page()
        
        await page.goto(url, wait_until='domcontentloaded', timeout=45000)
        await asyncio.sleep(15)
        
        for _ in range(3):
            await page.evaluate("window.scrollBy(0, 1000)")
            await asyncio.sleep(1)
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await asyncio.sleep(3)
        
        events = await self._extract_from_dom(page, url)
        
        logger.info(f"Olimp ({url}): Extracted {len(events)} events")
        
        await browser.close()
        return events
    
    async def _extract_from_dom(self, page, url: str) -> List[Dict]:
        raw_events = await page.evaluate("""
            () => {
                const results = [];
                
                const allText = document.body.innerText;
                const lines = allText.split(/\\n/);
                
                let currentTeams = [];
                
                for (let i = 0; i < lines.length; i++) {
                    const line = lines[i].trim();
                    if (!line) continue;
                    
                    const numbers = line.match(/\\d+[.,]\\d{1,3}/g) || [];
                    const odds = numbers
                        .map(n => parseFloat(n.replace(',', '.')))
                        .filter(v => v >= 1.01 && v <= 30);
                    
                    if (odds.length >= 2) {
                        const prevText = lines.slice(Math.max(0, i-10), i).join(' ');
                        
                        const teamMatches = prevText.match(/([A-ZА-ЯЁ][a-zа-яё]{1,20}(?:\\s+[A-ZА-ЯЁ][a-zа-яё]{1,20})*)/g) || [];
                        
                        let home = '', away = '';
                        if (teamMatches.length >= 2) {
                            home = teamMatches[0];
                            away = teamMatches[teamMatches.length - 1];
                        }
                        
                        if (home && away && home !== away) {
                            results.push({
                                home: home.trim(),
                                away: away.trim(),
                                odds: odds.slice(0, 3)
                            });
                        }
                    }
                }
                
                return results;
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
                'id': f"olimp_{i}_{hash(key) % 1000000}",
                'bookmaker': 'olimp',
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
    parser = OlimpPlaywrightParser()
    events = await parser.get_events()
    print(f'Found {len(events)} events from Olimp')
    for e in events[:5]:
        print(f"  {e['home_team']} vs {e['away_team']}: {e['home_odds']} - {e['draw_odds']} - {e['away_odds']}")

if __name__ == '__main__':
    asyncio.run(test())
