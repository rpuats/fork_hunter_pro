# scanner/parsers/fonbet_parser.py
"""
Fonbet parser - uses Fonbet's client API and fallback endpoints.
Fonbet provides live and pre-match events via their API.
"""
import time
import logging
from typing import List, Dict, Optional, Any
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class FonbetParser(BaseParser):
    name = "Fonbet"
    slug = "fonbet"
    base_url = "https://www.fonbet.ru"

    LIVE_API_URLS = [
        "https://api.fonbet.ru/live/v3/en/live/",
        "https://client-api.24h.bet/api/v2/client/line/live/football",
        "https://www.fonbet.ru/live/api/events",
        "https://api.fonbet.ru/live/v2/en/live/",
        "https://mobile.fonbet.ru/api/live/events",
    ]

    PREMATCH_API_URLS = [
        "https://api.fonbet.ru/live/v3/en/prematch/",
        "https://client-api.24h.bet/api/v2/client/line/prematch/football",
        "https://www.fonbet.ru/api/prematch/events",
    ]

    SPORT_IDS = {
        1: "football",
        2: "hockey",
        3: "basketball",
        4: "tennis",
        5: "volleyball",
        6: "baseball",
        7: "handball",
        8: "mma",
    }

    async def get_events(self) -> List[Dict]:
        events = []

        for is_live in [True, False]:
            urls = self.LIVE_API_URLS if is_live else self.PREMATCH_API_URLS
            headers = self._get_fonbet_headers()

            for url in urls:
                try:
                    data = await self.fetch(url, headers=headers)
                    if data:
                        parsed = self._parse_response(data, is_live=is_live)
                        if parsed:
                            events.extend(parsed)
                            logger.info(f"Fonbet: got {len(parsed)} events from {url}")
                            break
                except Exception as e:
                    logger.debug(f"Fonbet: failed to fetch {url}: {e}")
                    continue

        return events[:50]

    def _get_fonbet_headers(self) -> Dict[str, str]:
        headers = self._get_headers()
        headers.update({
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://www.fonbet.ru/",
            "Origin": "https://www.fonbet.ru",
            "X-Requested-With": "XMLHttpRequest",
        })
        return headers

    def _parse_response(self, data: Any, is_live: bool = True) -> List[Dict]:
        events = []

        if isinstance(data, dict):
            items = (
                data.get('events', [])
                or data.get('games', [])
                or data.get('data', [])
                or data.get('results', [])
            )
            if isinstance(items, dict):
                items = list(items.values())
        elif isinstance(data, list):
            items = data
        else:
            logger.warning(f"Fonbet: unexpected data type: {type(data)}")
            return []

        for item in items if isinstance(items, list) else []:
            try:
                event = self._normalize_event(item, is_live=is_live)
                if event:
                    events.append(event)
            except Exception as e:
                logger.debug(f"Fonbet: failed to normalize event: {e}")
                continue

        return events

    def _normalize_event(self, raw: Dict, is_live: bool = True) -> Optional[Dict]:
        try:
            name = raw.get('name', '') or raw.get('title', '')
            if ' - ' in name:
                home, away = name.split(' - ', 1)
            elif ':' in name:
                home, away = name.split(':', 1)
            else:
                home = raw.get('team1') or raw.get('home') or raw.get('competitor1', '')
                away = raw.get('team2') or raw.get('away') or raw.get('competitor2', '')

            home = str(home).strip()
            away = str(away).strip()

            if not home or not away:
                return None

            coeffs = raw.get('coeffs', {}) or raw.get('coefficients', {}) or raw.get('odds', {}) or raw.get('markets', {})

            if isinstance(coeffs, list):
                for market in coeffs:
                    if isinstance(market, dict):
                        if market.get('type') in ['1x2', 'main', 'match_winner', '1X2']:
                            coeffs = market.get('outcomes', market.get('odds', {}))
                            break

            home_odds = self._extract_odds(coeffs, ['w1', 'win1', '1', 'home', 'competitor1'])
            draw_odds = self._extract_odds(coeffs, ['draw', 'x', 'X', 'tie'])
            away_odds = self._extract_odds(coeffs, ['w2', 'win2', '2', 'away', 'competitor2'])

            if home_odds < 1.01 and away_odds < 1.01:
                return None

            sport_id = raw.get('sportId') or raw.get('sport_id') or raw.get('sport', 1)
            sport = self.SPORT_IDS.get(int(sport_id), 'football') if isinstance(sport_id, (int, str)) else 'football'

            league = (
                raw.get('category')
                or raw.get('league')
                or raw.get('tournament')
                or raw.get('champ')
                or raw.get('competition')
                or 'Live' if is_live else 'Prematch'
            )

            event_id = raw.get('id') or raw.get('eventId') or raw.get('gameId') or f"fonbet_{hash(home + away)}"

            return {
                'id': f"fonbet_{event_id}",
                'bookmaker': 'fonbet',
                'sport': sport,
                'home_team': home,
                'away_team': away,
                'league': str(league),
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': is_live,
                'market': '1x2',
                'source_url': f"{self.base_url}/{'live' if is_live else 'prematch'}",
                'scraped_at': time.time()
            }
        except Exception as e:
            logger.debug(f"Fonbet: error normalizing event: {e}")
            return None

    def _extract_odds(self, coeffs: Any, keys: List[str]) -> float:
        if not isinstance(coeffs, dict):
            return 0.0
        for key in keys:
            val = coeffs.get(key)
            if val is not None:
                try:
                    return float(val)
                except (ValueError, TypeError):
                    continue
        return 0.0
