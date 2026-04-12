# scanner/parsers/betcity_parser.py
import logging
import json
import re
from typing import List, Dict, Optional
from bs4 import BeautifulSoup
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class BetcityParser(BaseParser):
    name = "Betcity"
    slug = "betcity"
    base_url = "https://betcity.ru"
    
    async def get_events(self) -> List[Dict]:
        events = []

        urls = [
            "https://betcity.ru/ru/line/football",
        ]

        for url in urls:
            try:
                html = await self.fetch(url, json=False)
                if html:
                    parsed = self._parse_html(html, url)
                    if parsed:
                        events.extend(parsed)
                        logger.info(f"Betcity: got {len(parsed)} events from {url}")
                        break
            except Exception as e:
                logger.debug(f"Betcity: failed to fetch {url}: {e}")
                continue

        logger.debug(f"Betcity: total {len(events)} events collected")
        return events[:50]
    
    def _parse_html(self, html: str, url: str) -> List[Dict]:
        events = []
        soup = BeautifulSoup(html, 'html.parser')

        # First, try to extract from embedded JSON (React SPA data)
        json_events = self._extract_from_json(html)
        if json_events:
            events.extend(json_events)
            return events

        # Fallback to DOM parsing
        # Try to find event containers using various selectors
        selectors = [
            'tr[data-event]',
            '.event-row',
            '.match',
            'div[class*="event"]',
            'tr.g-tr',
            '[data-testid*="event"]'
        ]

        containers = []
        for selector in selectors:
            containers = soup.select(selector)
            if containers:
                logger.debug(f"Betcity: found {len(containers)} events using selector '{selector}'")
                break

        for container in containers:
            # Extract team names
            team_selectors = [
                'td.team-name',
                '.team',
                'span.team',
                'a.team',
                '[class*="team"]'
            ]
            teams = []
            for tsel in team_selectors:
                elems = container.select(tsel)
                if len(elems) >= 2:
                    teams = [elem.get_text().strip() for elem in elems if elem.get_text().strip()]
                    if len(teams) >= 2:
                        break
            if len(teams) < 2:
                continue

            # Extract odds
            odds_selectors = [
                'td.odds',
                '.coefficient',
                'span.odds',
                '[class*="odd"]',
                'button[class*="bet"]'
            ]
            odds = []
            for osel in odds_selectors:
                elems = container.select(osel)
                for elem in elems:
                    text = elem.get_text().strip()
                    val = self._parse_odds(text)
                    if val:
                        odds.append(val)
                if len(odds) >= 2:
                    break

            if len(odds) >= 2:
                raw_event = {
                    'home_team': teams[0],
                    'away_team': teams[1],
                    'home_odds': odds[0],
                    'draw_odds': odds[1] if len(odds) > 2 else None,
                    'away_odds': odds[2] if len(odds) > 2 else odds[1],
                    'is_live': 'live' in url,
                    'league': 'Pre-match',
                    'source_url': url
                }
                normalized = self._normalize_event(raw_event)
                if normalized:
                    events.append(normalized)

        return events

    def _extract_from_json(self, html: str) -> List[Dict]:
        """Extract events from embedded JSON in HTML (for React SPA)."""
        events = []

        # Look for common patterns in script tags
        patterns = [
            r'window\.__INITIAL_STATE__\s*=\s*({.+?});',
            r'window\.__NEXT_DATA__\s*=\s*({.+?});',
            r'window\.betcityData\s*=\s*({.+?});',
            r'__NUXT__\s*=\s*({.+?});',
        ]

        for pattern in patterns:
            match = re.search(pattern, html, re.DOTALL)
            if match:
                try:
                    data = json.loads(match.group(1))
                    logger.debug(f"Betcity: found JSON data with pattern {pattern}")
                    # Parse the JSON structure
                    parsed_events = self._parse_json_data(data)
                    if parsed_events:
                        events.extend(parsed_events)
                        break
                except json.JSONDecodeError:
                    continue

        # Also look for data attributes
        soup = BeautifulSoup(html, 'html.parser')
        data_elements = soup.find_all(attrs={'data-event': True})
        for elem in data_elements:
            try:
                event_data = json.loads(elem['data-event'])
                parsed = self._parse_event_data(event_data)
                if parsed:
                    events.append(parsed)
            except (json.JSONDecodeError, KeyError):
                continue

        return events

    def _parse_json_data(self, data: Dict) -> List[Dict]:
        """Parse the JSON data structure to extract events."""
        events = []
        if 'events' in data and isinstance(data['events'], list):
            for event in data['events']:
                parsed = self._parse_event_data(event)
                if parsed:
                    events.append(parsed)
        elif isinstance(data, dict):
            for key, value in data.items():
                if isinstance(value, list) and key in ['events', 'matches', 'games']:
                    for event in value:
                        parsed = self._parse_event_data(event)
                        if parsed:
                            events.append(parsed)
                elif isinstance(value, dict):
                    events.extend(self._parse_json_data(value))
        return events

    def _parse_event_data(self, event_data: Dict) -> Optional[Dict]:
        """Parse individual event data."""
        try:
            home_team = event_data.get('home_team') or event_data.get('home') or event_data.get('team1')
            away_team = event_data.get('away_team') or event_data.get('away') or event_data.get('team2')
            odds = event_data.get('odds', {})

            if not home_team or not away_team:
                return None

            home_odds = self._extract_odds(odds, ['home', '1', 'win1'])
            draw_odds = self._extract_odds(odds, ['draw', 'x', 'draw'])
            away_odds = self._extract_odds(odds, ['away', '2', 'win2'])

            if home_odds < 1.01 and away_odds < 1.01:
                return None

            return self._normalize_event({
                'home_team': home_team,
                'away_team': away_team,
                'home_odds': home_odds,
                'draw_odds': draw_odds,
                'away_odds': away_odds,
                'is_live': False,
                'league': event_data.get('league') or 'Pre-match',
                'source_url': self.base_url + '/line'
            })
        except Exception as e:
            logger.debug(f"Betcity: error parsing event data: {e}")
            return None

    def _is_odds(self, s: str) -> bool:
        try:
            val = float(s.replace(',', '.'))
            return 1.01 <= val <= 100
        except ValueError:
            return False

    def _parse_odds(self, s: str) -> Optional[float]:
        try:
            val = float(s.replace(',', '.'))
            if 1.01 <= val <= 100:
                return val
            return None
        except ValueError:
            return None

    def _normalize_event(self, raw: Dict) -> Optional[Dict]:
        try:
            home = raw.get('home_team') or raw.get('team1') or raw.get('home', 'Home')
            away = raw.get('away_team') or raw.get('team2') or raw.get('away', 'Away')

            home = str(home).strip()
            away = str(away).strip()

            if not home or not away:
                return None

            home_odds = float(raw.get('home_odds') or raw.get('k1') or raw.get('win1') or raw.get('coefficient1', 0))
            draw_odds = float(raw.get('draw_odds') or raw.get('kx') or raw.get('draw') or raw.get('coefficientX', 0))
            away_odds = float(raw.get('away_odds') or raw.get('k2') or raw.get('win2') or raw.get('coefficient2', 0))

            if home_odds < 1.01 and away_odds < 1.01:
                return None

            return {
                'id': f"betcity_{raw.get('id', hash(home + away))}",
                'bookmaker': 'betcity',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': raw.get('league') or raw.get('champ') or 'Live',
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': raw.get('is_live', True),
                'market': '1x2',
                'source_url': raw.get('source_url', f"{self.base_url}/line")
            }
        except Exception as e:
            logger.debug(f"Betcity: error normalizing event: {e}")
            return None
