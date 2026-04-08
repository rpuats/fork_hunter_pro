# scanner/parsers/leon_playwright.py
"""
Leon Playwright Parser - Real data extraction from SPA
Uses innerText for better extraction
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class LeonPlaywrightParser:
    name = "Leon (Playwright)"
    slug = "leon"
    base_url = "https://leon.ru/live/football"
    
    async def get_events(self) -> List[Dict]:
        max_retries = 2
        for attempt in range(max_retries):
            try:
                return await self._fetch_with_playwright()
            except Exception as e:
                logger.warning(f"Leon attempt {attempt + 1}/{max_retries} failed: {e}")
                if attempt < max_retries - 1:
                    await asyncio.sleep(3)
                continue
        return []
    
    async def _fetch_with_playwright(self) -> List[Dict]:
        pw = await async_playwright().start()
        self.browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled'])
        self.context = await self.browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await self.context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
            window.chrome = { runtime: {} };
        """)
        
        self.page = await self.context.new_page()
        await self.page.goto(self.base_url, wait_until='domcontentloaded', timeout=30000)
        await asyncio.sleep(15)
        
        await self.page.evaluate('window.scrollTo(0, document.body.scrollHeight / 2)')
        await asyncio.sleep(3)
        await self.page.evaluate('window.scrollTo(0, document.body.scrollHeight)')
        await asyncio.sleep(3)
        
        events = await self._extract_events()
        logger.info(f"Leon: Extracted {len(events)} events")
        
        await self.browser.close()
        return events
    
    async def _extract_events(self) -> List[Dict]:
        raw_events = await self.page.evaluate("""
            () => {
                const bodyText = document.body.innerText;
                const lines = bodyText.split(/\\n/);
                
                const events = [];
                
                for (let i = 0; i < lines.length; i++) {
                    const line = lines[i].trim();
                    if (!line) continue;
                    
                    const odds = line.match(/\\d+[.,]\\d+/g) || [];
                    const validOdds = odds.map(o => parseFloat(o.replace(',', '.'))).filter(v => v >= 1.01 && v <= 50);
                    
                    if (validOdds.length >= 2) {
                        const otherLines = lines.slice(Math.max(0, i-5), i).join(' ');
                        const teams = otherLines.match(/[A-ZА-ЯЁ][a-zа-яё]+(?:\\s+[A-ZА-ЯЁ][a-zа-яё]+)*/g) || [];
                        
                        if (teams.length >= 2) {
                            events.push({
                                home: teams[0],
                                away: teams[teams.length - 1],
                                odds: validOdds.slice(0, 3)
                            });
                        }
                    }
                }
                
                return events;
            }
        """)
        
        result = []
        seen = set()
        for i, e in enumerate(raw_events):
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            
            if not home or not away:
                continue
            if len(home) < 2 or len(away) < 2:
                continue
            
            key = f"{home}|{away}"
            if key in seen:
                continue
            seen.add(key)
            
            if len(odds) < 2:
                continue
            
            is_3way = len(odds) >= 3
            
            event = {
                'id': f"leon_{i}_{hash(key) % 1000000}",
                'bookmaker': 'leon',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': 'Live',
                'home_odds': odds[0],
                'draw_odds': odds[1] if is_3way else None,
                'away_odds': odds[2] if is_3way else odds[1],
                'is_live': True,
                'market': '1x2',
                'source_url': self.base_url,
                'scraped_at': time.time()
            }
            
            if event['home_odds'] >= 1.01:
                result.append(event)
        
        return result
