# scanner/parsers/baltbet_playwright.py
"""
Baltbet Playwright Parser
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright
from scanner.parsers.stealth import create_stealth_context, generate_stealth_config

logger = logging.getLogger(__name__)


class BaltbetPlaywrightParser:
    name = "Baltbet (Playwright)"
    slug = "baltbet"
    urls = [
        "https://baltbet.ru/live",
        "https://baltbet.ru/line",
        "https://baltbet.ru/live/basketball",
        "https://baltbet.ru/line/basketball",
        "https://baltbet.ru/live/hockey",
        "https://baltbet.ru/line/hockey",
        "https://baltbet.ru/live/tennis",
        "https://baltbet.ru/line/tennis",
        "https://baltbet.ru/live/volleyball",
        "https://baltbet.ru/line/volleyball",
        "https://baltbet.ru/live/baseball",
        "https://baltbet.ru/line/baseball",
        "https://baltbet.ru/live/handball",
        "https://baltbet.ru/line/handball",
        "https://baltbet.ru/live/rugby",
        "https://baltbet.ru/line/rugby",
        "https://baltbet.ru/live/table-tennis",
        "https://baltbet.ru/line/table-tennis",
        "https://baltbet.ru/live/badminton",
        "https://baltbet.ru/line/badminton",
    ]
    
    GENERIC_WORDS = {
        "футбол", "счёт", "счет", "live", "лайв", "матч", "игра", "спорт",
        "football", "soccer", "sport", "game", "match", "count",
        "basketball", "теннис", "hockey", "хоккей", "volleyball",
        "волейбол", "статистика", "statistics", "время", "time",
        "vs", "против", "команда", "team", "total", "тотал",
    }
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        for url_idx, url in enumerate(self.urls):
            try:
                events = await self._fetch_url(url)
                all_events.extend(events)
                if len(all_events) > 0 and url_idx >= 3:
                    logger.info(f"Baltbet: early break after {url_idx + 1} URLs, {len(all_events)} events")
                    break
            except Exception as e:
                logger.warning(f"Baltbet failed for {url}: {e}")
                continue
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
        await page.goto(url, wait_until='domcontentloaded', timeout=15000)
        await asyncio.sleep(3)
        
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
        await asyncio.sleep(1)
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await asyncio.sleep(1)
        
        events = await self._extract_events(page, url)
        logger.info(f"Baltbet ({url}): Extracted {len(events)} events")
        
        await browser.close()
        return events
    
    async def _extract_events(self, page, url: str) -> List[Dict]:
        raw_events = await page.evaluate("""
            () => {
                const events = [];
                const containers = document.querySelectorAll('[class*="event"], [class*="match"], [class*="game"], .sport-event, .event-line');
                
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
                            home: teams[0],
                            away: teams[1],
                            odds: odds.slice(0, 4)
                        });
                    }
                });
                
                return events;
            }
        """)
        
        return self._normalize_events(raw_events, url)
    
    def _is_valid_team_name(self, name: str) -> bool:
        """Validate team name quality."""
        if not name or len(name.strip()) < 2:
            return False
        
        name_stripped = name.strip()
        name_lower = name_stripped.lower()
        
        # Check for generic/sport words
        if any(word in name_lower for word in self.GENERIC_WORDS):
            return False
        
        # Check if name is purely numeric
        if name_stripped.replace('.', '').replace(',', '').replace(' ', '').isdigit():
            return False
        
        # Check for common placeholder patterns
        placeholder_patterns = ['team ', 'команда ', 'player', 'игрок', 'unknown', 'неизвест']
        if any(name_lower.startswith(p) for p in placeholder_patterns):
            return False
        
        return True
    
    def _normalize_events(self, raw_events: list, url: str) -> List[Dict]:
        result = []
        seen = set()
        
        for i, e in enumerate(raw_events):
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            
            # Validate team names
            if not self._is_valid_team_name(home) or not self._is_valid_team_name(away):
                continue
            
            # Teams must be different
            if home.lower() == away.lower():
                continue
            
            key = f"{home}|{away}"
            if key in seen:
                continue
            seen.add(key)
            
            if len(odds) < 2:
                continue
            
            is_3way = len(odds) >= 3
            
            totals_over = e.get('totals_over', {}) or {}
            totals_under = e.get('totals_under', {}) or {}
            
            event = {
                'id': f"baltbet_{i}_{hash(key) % 1000000}",
                'bookmaker': 'baltbet',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': 'Live' if 'live' in url else 'Pre-match',
                'home_odds': odds[0],
                'draw_odds': odds[1] if is_3way else None,
                'away_odds': odds[2] if is_3way else odds[1],
                'is_live': 'live' in url,
                'market': '1x2',
                'total_over': totals_over,
                'total_under': totals_under,
                'source_url': url,
                'scraped_at': time.time()
            }
            
            if event['home_odds'] >= 1.01:
                result.append(event)
        
        return result


async def test():
    logging.basicConfig(level=logging.INFO)
    parser = BaltbetPlaywrightParser()
    events = await parser.get_events()
    print(f'Baltbet: {len(events)} events')

if __name__ == '__main__':
    asyncio.run(test())
