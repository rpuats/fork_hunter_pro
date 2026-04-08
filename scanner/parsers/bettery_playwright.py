# scanner/parsers/bettery_playwright.py
"""
Bettery Playwright Parser
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright
from scanner.parsers.stealth import create_stealth_context, generate_stealth_config

logger = logging.getLogger(__name__)


class BetteryPlaywrightParser:
    name = "Bettery (Playwright)"
    slug = "bettery"
    urls = [
        "https://www.bettery.ru/live",
        "https://www.bettery.ru/line",
        "https://www.bettery.ru/live/basketball",
        "https://www.bettery.ru/line/basketball",
        "https://www.bettery.ru/live/hockey",
        "https://www.bettery.ru/line/hockey",
        "https://www.bettery.ru/live/tennis",
        "https://www.bettery.ru/line/tennis",
        "https://www.bettery.ru/live/volleyball",
        "https://www.bettery.ru/line/volleyball",
        "https://www.bettery.ru/live/baseball",
        "https://www.bettery.ru/line/baseball",
        "https://www.bettery.ru/live/handball",
        "https://www.bettery.ru/line/handball",
        "https://www.bettery.ru/live/rugby",
        "https://www.bettery.ru/line/rugby",
        "https://www.bettery.ru/live/table-tennis",
        "https://www.bettery.ru/line/table-tennis",
        "https://www.bettery.ru/live/badminton",
        "https://www.bettery.ru/line/badminton",
    ]
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        for url_idx, url in enumerate(self.urls):
            try:
                events = await self._fetch_url(url)
                all_events.extend(events)
                if len(all_events) > 0 and url_idx >= 3:
                    logger.info(f"Bettery: early break after {url_idx + 1} URLs, {len(all_events)} events")
                    break
            except Exception as e:
                logger.warning(f"Bettery failed for {url}: {e}")
                continue
        return all_events
    
    async def _fetch_url(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(headless=True)
        config = generate_stealth_config()
        context = await create_stealth_context(browser, config)
        
        page = await context.new_page()
        await page.goto(url, wait_until='domcontentloaded', timeout=15000)
        await asyncio.sleep(3)
        
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
        await asyncio.sleep(1)
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await asyncio.sleep(1)
        
        events = await self._extract_events(page, url)
        logger.info(f"Bettery ({url}): Extracted {len(events)} events")
        
        await browser.close()
        return events
    
    async def _extract_events(self, page, url: str) -> List[Dict]:
        # Use body text approach - scan all text for patterns
        raw_events = await page.evaluate("""
            () => {
                const events = [];
                const text = document.body.innerText;
                const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                
                // Look for patterns: team names followed by odds
                let i = 0;
                while (i < lines.length - 2) {
                    const line = lines[i];
                    const nextLine = lines[i + 1];
                    
                    // Check if current line looks like a team name
                    if (line.length > 3 && line.length < 50 && 
                        !line.match(/^\\d/) && !line.match(/LIVE|live/i) &&
                        !line.match(/^\\d+[.,]\\d+$/)) {
                        
                        // Look for odds in next few lines
                        const odds = [];
                        let j = i + 1;
                        const teamLines = [line];
                        
                        while (j < lines.length && odds.length < 6) {
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
                                odds: odds.slice(0, 4)
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
                'id': f"bettery_{i}_{hash(key) % 1000000}",
                'bookmaker': 'bettery',
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
    parser = BetteryPlaywrightParser()
    events = await parser.get_events()
    print(f'Bettery: {len(events)} events')

if __name__ == '__main__':
    asyncio.run(test())
