# scanner/parsers/olimpbet_playwright.py
"""
OlimpBet Playwright Parser - Real data extraction from SPA
Uses network intercept to capture API calls
"""
import asyncio
import time
import json
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class OlimpBetPlaywrightParser:
    name = "OlimpBet (Playwright)"
    slug = "olimpbet"
    urls = [
        "https://olimp.bet/live/football",
        "https://olimp.bet/line/football",
        "https://olimp.bet/live/basketball",
        "https://olimp.bet/line/basketball",
        "https://olimp.bet/live/hockey",
        "https://olimp.bet/line/hockey",
        "https://olimp.bet/live/tennis",
        "https://olimp.bet/line/tennis",
        "https://olimp.bet/live/volleyball",
        "https://olimp.bet/line/volleyball",
        "https://olimp.bet/live/baseball",
        "https://olimp.bet/line/baseball",
        "https://olimp.bet/live/handball",
        "https://olimp.bet/line/handball",
        "https://olimp.bet/live/rugby",
        "https://olimp.bet/line/rugby",
        "https://olimp.bet/live/table-tennis",
        "https://olimp.bet/line/table-tennis",
        "https://olimp.bet/live/badminton",
        "https://olimp.bet/line/badminton",
    ]
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        for url_idx, url in enumerate(self.urls):
            try:
                events = await self._fetch_url(url)
                all_events.extend(events)
                if len(all_events) > 0 and url_idx >= 3:
                    logger.info(f"OlimpBet: early break after {url_idx + 1} URLs, {len(all_events)} events")
                    break
            except Exception as e:
                logger.warning(f"OlimpBet failed for {url}: {e}")
                continue
        return all_events
    
    async def _fetch_url(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(headless=True, args=['--disable-blink-features=AutomationControlled'])
        context = await browser.new_context(user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36', viewport={'width': 1920, 'height': 1080}, locale='ru-RU')
        await context.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined}); window.chrome = {runtime: {}};")
        page = await context.new_page()
        try:
            await page.goto(url, wait_until='domcontentloaded', timeout=15000)
            await asyncio.sleep(3)
            for i in range(3):
                await page.evaluate(f"window.scrollTo(0, document.body.scrollHeight / 3 * {i+1})")
                await asyncio.sleep(0.5)
            events = await self._extract_events(page, url)
            logger.info(f"OlimpBet ({url}): {len(events)} events")
        except Exception as e:
            logger.warning(f"OlimpBet error: {e}")
            events = []
        finally:
            await browser.close()
        return events
    
    async def _extract_events(self, page, url: str) -> List[Dict]:
        try:
            raw_events = await page.evaluate("""() => {
                const events = [];
                const text = document.body.innerText;
                const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                let i = 0;
                while (i < lines.length - 2) {
                    const line = lines[i];
                    if (line.length > 3 && line.length < 50 && !line.match(/^\\d/) && !line.match(/LIVE|live/i) && !line.match(/^\\d+[.,]\\d+$/) && !line.match(/^\\d+:\\d/)) {
                        const odds = [];
                        const teamLines = [line];
                        let j = i + 1;
                        while (j < lines.length && odds.length < 8) {
                            const val = parseFloat(lines[j].replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 50) { odds.push(val); }
                            else if (lines[j].length > 3 && lines[j].length < 50 && !lines[j].match(/^\\d/) && teamLines.length < 2) { teamLines.push(lines[j]); }
                            j++;
                            if (odds.length >= 2 && teamLines.length >= 2) break;
                        }
                        if (teamLines.length >= 2 && odds.length >= 1) {
                            events.push({home: teamLines[0], away: teamLines[teamLines.length - 1], odds: odds.slice(0, 5)});
                            i = j; continue;
                        }
                    }
                    i++;
                }
                return events;
            }""")
            return self._normalize(raw_events, url)
        except Exception as e:
            logger.warning(f"OlimpBet extraction error: {e}")
            return []
    
    def _normalize(self, raw_events: list, url: str) -> List[Dict]:
        result, seen = [], set()
        for e in raw_events:
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            if not home or not away or len(home) < 2 or len(away) < 2 or len(odds) < 1: continue
            key = f"{home}|{away}"
            if key in seen: continue
            seen.add(key)
            result.append({'id': f"olimpbet_{hash(key) % 1000000}", 'bookmaker': 'olimpbet', 'sport': 'football', 'home_team': home, 'away_team': away, 'league': 'Live' if 'live' in url else 'Pre-match', 'home_odds': odds[0], 'draw_odds': odds[1] if len(odds) > 2 else None, 'away_odds': odds[2] if len(odds) > 2 else (odds[1] if len(odds) > 1 else 0), 'is_live': 'live' in url, 'market': '1x2', 'source_url': url, 'scraped_at': time.time()})
        return result


async def test():
    logging.basicConfig(level=logging.INFO)
    parser = OlimpBetPlaywrightParser()
    events = await parser.get_events()
    print(f'Found {len(events)} events from OlimpBet')
    for e in events[:5]:
        print(f"  {e['home_team']} vs {e['away_team']}: {e['home_odds']} - {e['draw_odds']} - {e['away_odds']}")

if __name__ == '__main__':
    asyncio.run(test())
