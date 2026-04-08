# scanner/parsers/melbet_intercept.py
"""
Melbet Parser - Playwright with Network Interception
Melbet is SPA with no public API. We intercept network requests to get JSON data.
"""
import asyncio
import json
import time
import logging
from typing import List, Dict, Optional
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class MelbetInterceptParser:
    name = "Melbet (Intercept)"
    slug = "melbet"
    urls = [
        "https://melbet.ru/live",
        "https://melbet.ru/line",
    ]

    def __init__(self):
        self.events = []
        self._api_responses = []

    async def get_events(self) -> List[Dict]:
        all_events = []
        for url in self.urls:
            try:
                events = await self._fetch_with_intercept(url)
                all_events.extend(events)
            except Exception as e:
                logger.warning(f"Melbet failed for {url}: {e}")
        return all_events

    async def _fetch_with_intercept(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(
            headless=True,
            args=['--disable-blink-features=AutomationControlled']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
            Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
            Object.defineProperty(navigator, 'languages', { get: () => ['ru-RU', 'ru', 'en'] });
            window.chrome = { runtime: {} };
        """)

        page = await context.new_page()
        self._api_responses = []

        # Intercept all responses
        async def handle_response(response):
            try:
                ct = response.headers.get('content-type', '')
                if 'json' in ct and response.status == 200:
                    try:
                        data = await response.json()
                        url = response.url
                        if any(kw in url.lower() for kw in ['event', 'match', 'live', 'line', 'odds', 'sport', 'bet']):
                            self._api_responses.append({'url': url, 'data': data})
                    except:
                        pass
            except:
                pass

        page.on('response', handle_response)

        await page.goto(url, wait_until='domcontentloaded', timeout=60000)
        await asyncio.sleep(15)

        # Scroll to trigger lazy loading
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 3)")
        await asyncio.sleep(3)
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 2)")
        await asyncio.sleep(3)
        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
        await asyncio.sleep(3)

        # Try to extract events from page state
        events = await self._extract_from_page(page, url)

        # Also try from intercepted API responses
        if not events:
            events = await self._extract_from_api_responses(url)

        logger.info(f"Melbet ({url}): Extracted {len(events)} events, {len(self._api_responses)} API responses")

        await browser.close()
        return events

    async def _extract_from_page(self, page, url: str) -> List[Dict]:
        """Extract events from page DOM as fallback"""
        try:
            raw_events = await page.evaluate("""
                () => {
                    const events = [];
                    // Try common selectors for betting sites
                    const selectors = [
                        '[class*="event"]', '[class*="match"]', '[class*="game"]',
                        '[class*="sport"]', '[class*="live"]', '[class*="coeff"]',
                        '[class*="odds"]', '[class*="bet"]', '[class*="market"]'
                    ];

                    const allEls = [];
                    selectors.forEach(sel => {
                        document.querySelectorAll(sel).forEach(el => {
                            if (el.children.length > 0 && el.textContent.length > 20) {
                                allEls.push(el);
                            }
                        });
                    });

                    // Deduplicate
                    const seen = new Set();
                    allEls.forEach(el => {
                        const text = el.textContent.trim().substring(0, 200);
                        if (!seen.has(text) && text.length > 20) {
                            seen.add(text);
                            events.push(text);
                        }
                    });

                    return events;
                }
            """)

            events = []
            for text in raw_events[:100]:
                lines = [l.strip() for l in text.split('\n') if l.strip()]
                teams = []
                odds = []
                for line in lines:
                    try:
                        val = float(line.replace(',', '.'))
                        if 1.01 <= val <= 100:
                            odds.append(val)
                    except:
                        if len(line) > 2 and len(line) < 50:
                            teams.append(line)

                if len(teams) >= 2 and len(odds) >= 2:
                    events.append({
                        'id': f"melbet_{hash(teams[0] + teams[1])}",
                        'bookmaker': 'melbet',
                        'sport': 'football',
                        'home_team': teams[0],
                        'away_team': teams[1],
                        'league': 'Live',
                        'home_odds': odds[0] if len(odds) > 0 else 0,
                        'draw_odds': odds[1] if len(odds) > 1 else None,
                        'away_odds': odds[2] if len(odds) > 2 else (odds[1] if len(odds) > 1 else 0),
                        'is_live': 'live' in url,
                        'market': '1x2',
                        'source_url': url,
                        'scraped_at': time.time()
                    })

            return events
        except Exception as e:
            logger.warning(f"Melbet page extraction failed: {e}")
            return []

    async def _extract_from_api_responses(self, url: str) -> List[Dict]:
        """Extract events from intercepted API responses"""
        events = []
        for resp in self._api_responses:
            data = resp['data']
            if isinstance(data, dict):
                items = data.get('events', []) or data.get('matches', []) or data.get('data', [])
                if isinstance(items, list):
                    for item in items:
                        if isinstance(item, dict):
                            e = self._parse_api_event(item, url)
                            if e:
                                events.append(e)
            elif isinstance(data, list):
                for item in data:
                    if isinstance(item, dict):
                        e = self._parse_api_event(item, url)
                        if e:
                            events.append(e)
        return events

    def _parse_api_event(self, raw: Dict, url: str) -> Optional[Dict]:
        try:
            home = raw.get('home') or raw.get('homeTeam') or raw.get('team1') or ''
            away = raw.get('away') or raw.get('awayTeam') or raw.get('team2') or ''
            if not home or not away:
                return None

            home_odds = float(raw.get('homeOdds', raw.get('k1', raw.get('c1', 0))))
            draw_odds = float(raw.get('drawOdds', raw.get('kx', raw.get('cX', 0))))
            away_odds = float(raw.get('awayOdds', raw.get('k2', raw.get('c2', 0))))

            if home_odds < 1.01 and away_odds < 1.01:
                return None

            return {
                'id': f"melbet_{raw.get('id', hash(str(home) + str(away)))}",
                'bookmaker': 'melbet',
                'sport': raw.get('sport', 'football'),
                'home_team': str(home),
                'away_team': str(away),
                'league': raw.get('league', 'Live'),
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': 'live' in url,
                'market': '1x2',
                'source_url': url,
                'scraped_at': time.time()
            }
        except:
            return None
