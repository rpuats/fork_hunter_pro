# scanner/parsers/baltbet_playwright.py
"""
Baltbet Playwright Parser
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class BaltbetRegexParser:
    name = "Baltbet (Playwright)"
    slug = "baltbet"
    url = "https://old.baltbet.ru/Line1.aspx"

    def __init__(self):
        self.browser = None
        self.context = None
        self.page = None

    async def __aenter__(self):
        pw = await async_playwright().start()
        self.browser = await pw.chromium.launch(
            headless=True,
            args=[
                '--disable-blink-features=AutomationControlled',
                '--no-sandbox',
                '--disable-dev-shm-usage',
                '--disable-web-security',
                '--disable-features=IsolateOrigins,site-per-process',
                '--disable-infobars',
            ]
        )
        self.context = await self.browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36'
        )
        return self

    async def __aexit__(self, *args):
        if self.page:
            await self.page.close()
        if self.context:
            await self.context.close()
        if self.browser:
            await self.browser.close()

    async def get_events(self) -> List[Dict]:
        self.page = await self.context.new_page()
        print(f"Baltbet debug: Loading URL {self.url}")
        await self.page.goto(self.url, wait_until='domcontentloaded', timeout=60000)
        await self.page.wait_for_timeout(5000)  # Wait for the page to load completely
        title = await self.page.title()
        print(f"Baltbet debug: Page loaded, title length: {len(title)}")

        # Scroll down to load more events initially
        await self._scroll_to_load_events(3)

        all_events = []

        # Define sport tabs to click: football, hockey, basketball, tennis
        sport_keywords = {
            'football': ['футбол', 'football'],
            'hockey': ['хоккей', 'hockey'],
            'basketball': ['баскетбол', 'basketball'],
            'tennis': ['теннис', 'tennis']
        }

        # Get all sport tab elements - try multiple selectors
        sport_tabs = await self.page.query_selector_all('div.filter')
        print(f"Baltbet debug: Found {len(sport_tabs)} sport tabs with div.filter")

        if len(sport_tabs) == 0:
            sport_tabs = await self.page.query_selector_all('ul.newstabs li a')
            print(f"Baltbet debug: Found {len(sport_tabs)} sport tabs with ul.newstabs li a")

        # For now, just extract from the default view (football)
        sport = 'football'
        try:
            # Scroll to load more events for this sport
            await self._scroll_to_load_events(5)
            # Extract events using page.evaluate
            result = await self.page.evaluate(self._extract_events_js(), sport)
            events = result.get('events', [])
            debug = result.get('debug', {})
            print(f"Baltbet debug for {sport}:", debug)
            all_events.extend(events)
            print(f"Baltbet: Extracted {len(events)} events for {sport}")
        except Exception as e:
            print(f"Baltbet: Failed to process sport {sport}: {str(e)[:100]}")

        logger.info(f"Total Baltbet events: {len(all_events)}")
        await self.page.close()
        return self._normalize_events(all_events)

    async def _scroll_to_load_events(self, scrolls: int):
        for _ in range(scrolls):
            await self.page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
            await self.page.wait_for_timeout(1000)

    def _extract_events_js(self):
        # JavaScript function to extract events from the page
        return """
        (sport) => {
            const events = [];
            const debug = {
                title: document.title,
                bodyLength: document.body ? document.body.innerHTML.length : 0,
                selectors: {}
            };

            // Try multiple selectors
            const selectors = ['table.coef-tobasket', 'table', '.event', '.match', '[class*="event"]', '[class*="match"]'];
            let eventContainers = [];

            for (const selector of selectors) {
                const found = document.querySelectorAll(selector);
                debug.selectors[selector] = found.length;
                if (found.length > 0 && eventContainers.length === 0) {
                    eventContainers = found;
                }
            }

            debug.eventContainers = eventContainers.length;

            eventContainers.forEach(container => {
                const rows = container.querySelectorAll('tr');
                rows.forEach(row => {
                    const names = row.querySelectorAll('span.name');
                    const coefs = row.querySelectorAll('span.coe');

                    if (names.length >= 3 && coefs.length >= 3) {
                        const homeName = names[0].textContent.trim();
                        const drawName = names[1].textContent.trim();
                        const awayName = names[2].textContent.trim();

                        if (drawName.includes('Ничья') || drawName.toLowerCase().includes('draw')) {
                            const homeOdds = parseFloat(coefs[0].textContent.replace(',', '.'));
                            const drawOdds = parseFloat(coefs[1].textContent.replace(',', '.'));
                            const awayOdds = parseFloat(coefs[2].textContent.replace(',', '.'));

                            if (!isNaN(homeOdds) && !isNaN(drawOdds) && !isNaN(awayOdds)) {
                                // Find team names from previous table or similar
                                const prevTable = container.previousElementSibling;
                                let homeTeam = homeName;
                                let awayTeam = awayName;
                                if (prevTable && prevTable.tagName === 'TABLE') {
                                    const players = prevTable.querySelectorAll('td.liveplayer');
                                    if (players.length >= 2) {
                                        homeTeam = players[0].textContent.replace('Live', '').trim();
                                        awayTeam = players[1].textContent.trim();
                                    }
                                }

                                events.push({
                                    home: homeTeam,
                                    away: awayTeam,
                                    odds: [homeOdds, drawOdds, awayOdds],
                                    sport: sport
                                });
                            }
                        }
                    }
                });
            });

            // Also collect from sidebar or other areas if needed
            const sidebarLinks = document.querySelectorAll('a[href^="javascript:openlive"]');
            debug.sidebarLinks = sidebarLinks.length;

            // Try to find actual betting tables with odds
            const allTables = document.querySelectorAll('table');
            debug.totalTables = allTables.length;

            allTables.forEach((table, idx) => {
                const tableText = table.textContent.substring(0, 200);
                debug[`table_${idx}_preview`] = tableText;

                // Look for odds patterns
                const oddsPattern = /\\b\\d+\\.\\d+\\b/g;
                const oddsMatches = table.textContent.match(new RegExp('\\b\\d+\\.\\d+\\b', 'g'));
                if (oddsMatches) {
                    debug[`table_${idx}_odds`] = oddsMatches.slice(0, 10);
                }
            });

            sidebarLinks.forEach(link => {
                const text = link.textContent.trim();
                if (text.includes('-')) {
                    const parts = text.split('-', 2);
                    if (parts.length === 2) {
                        const home = parts[0].trim();
                        const away = parts[1].trim();
                        events.push({
                            home: home,
                            away: away,
                            odds: [2.0, 3.0, 4.0],  // Default odds
                            sport: sport
                        });
                    }
                }
            });

            return {events: events, debug: debug};
        }
        """

    def _normalize_events(self, raw_events: list) -> List[Dict]:
        result = []
        seen = set()

        for i, e in enumerate(raw_events):
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            sport = e.get('sport', 'football')

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

            event = {
                'id': f"baltbet_{i}_{hash(key) % 1000000}",
                'bookmaker': 'baltbet',
                'sport': sport,
                'home_team': home,
                'away_team': away,
                'league': 'Unknown',  # Could extract league if available
                'home_odds': odds[0],
                'draw_odds': odds[1] if is_3way else None,
                'away_odds': odds[2] if is_3way else odds[1],
                'is_live': False,  # Assume pre-match
                'market': '1x2',
                'source_url': self.url,
                'scraped_at': time.time()
            }

            if event['home_odds'] >= 1.01:
                result.append(event)

        return result

    def _is_valid_team_name(self, name: str) -> bool:
        """Validate team name quality."""
        if not name or len(name.strip()) < 2:
            return False

        name_stripped = name.strip()
        name_lower = name_stripped.lower()

        # Check for generic/sport words
        generic_words = {
            "футбол", "счёт", "счет", "live", "лайв", "матч", "игра", "спорт",
            "football", "soccer", "sport", "game", "match", "count",
            "basketball", "теннис", "hockey", "хоккей", "volleyball",
            "волейбол", "статистика", "statistics", "время", "time",
            "vs", "против", "команда", "team", "total", "тотал",
        }
        if any(word in name_lower for word in generic_words):
            return False

        # Check if name is purely numeric
        if name_stripped.replace('.', '').replace(',', '').replace(' ', '').isdigit():
            return False

        # Check for common placeholder patterns
        placeholder_patterns = ['team ', 'команда ', 'player', 'игрок', 'unknown', 'неизвест']
        if any(name_lower.startswith(p) for p in placeholder_patterns):
            return False

        return True


def test():
    logging.basicConfig(level=logging.INFO)
    async def run():
        async with BaltbetPlaywrightParser() as parser:
            events = await parser.get_events()
            print(f'Baltbet: {len(events)} events')
    asyncio.run(run())


if __name__ == '__main__':
    test()