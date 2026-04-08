# scanner/parsers/pari_parser.py
"""
Pari parser - uses Pari's API endpoints with multiple fallback URLs.
Pari provides live and pre-match events via REST API.
"""
import time
import logging
from typing import List, Dict, Optional, Any
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class PariParser(BaseParser):
    name = "Pari"
    slug = "pari"
    base_url = "https://www.pari.ru"

    LIVE_API_URLS = [
        "https://www.pari.ru/LiveFeed/GetGamesHtml",
        "https://www.pari.ru/api/live/v2/events",
        "https://api.pari.ru/live/events",
        "https://www.pari.ru/live/api/getGames",
        "https://mobile.pari.ru/api/live/sports",
    ]

    PREMATCH_API_URLS = [
        "https://www.pari.ru/api/prematch/v2/events",
        "https://api.pari.ru/prematch/events",
        "https://www.pari.ru/prematch/api/getGames",
    ]

    SPORT_MAP = {
        "football": "football",
        "soccer": "football",
        "hockey": "hockey",
        "basketball": "basketball",
        "tennis": "tennis",
        "volleyball": "volleyball",
        "mma": "mma",
        "boxing": "boxing",
        "esports": "esports",
    }

    async def get_events(self) -> List[Dict]:
        events = []

        for is_live in [True, False]:
            urls = self.LIVE_API_URLS if is_live else self.PREMATCH_API_URLS
            headers = self._get_pari_headers()

            for url in urls:
                try:
                    data = await self.fetch(url, headers=headers)
                    if data:
                        parsed = self._parse_response(data, is_live=is_live)
                        if parsed:
                            events.extend(parsed)
                            logger.info(f"Pari: got {len(parsed)} events from {url}")
                            break
                except Exception as e:
                    logger.debug(f"Pari: failed to fetch {url}: {e}")
                    continue

        return events[:50]

    def _get_pari_headers(self) -> Dict[str, str]:
        headers = self._get_headers()
        headers.update({
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://www.pari.ru/",
            "Origin": "https://www.pari.ru",
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
                or data.get('matches', [])
            )
            if isinstance(items, dict):
                items = list(items.values())
        elif isinstance(data, list):
            items = data
        else:
            logger.warning(f"Pari: unexpected data type: {type(data)}")
            return []

        for item in items if isinstance(items, list) else []:
            try:
                event = self._normalize_event(item, is_live=is_live)
                if event:
                    events.append(event)
            except Exception as e:
                logger.debug(f"Pari: failed to normalize event: {e}")
                continue

        return events

    def _normalize_event(self, raw: Dict, is_live: bool = True) -> Optional[Dict]:
        try:
            home = (
                raw.get('team1')
                or raw.get('home')
                or raw.get('homeTeam')
                or raw.get('competitor1')
                or ''
            )
            away = (
                raw.get('team2')
                or raw.get('away')
                or raw.get('awayTeam')
                or raw.get('competitor2')
                or ''
            )

            home = str(home).strip()
            away = str(away).strip()

            if not home or not away:
                return None

            coeffs = (
                raw.get('k', {})
                or raw.get('odds', {})
                or raw.get('coefficients', {})
                or raw.get('markets', {})
                or raw.get('marketsData', {})
            )

            if isinstance(coeffs, list):
                for market in coeffs:
                    if isinstance(market, dict):
                        market_type = market.get('type', '').lower()
                        market_name = market.get('name', '').lower()
                        if market_type in ['1x2', 'main', 'match_winner', '1х2'] or market_name in ['main', '1x2', 'match result']:
                            coeffs = market.get('outcomes', market.get('odds', market.get('selections', {})))
                            break

            home_odds = self._extract_odds(coeffs, ['k1', 'win1', 'w1', 'coefficient1', '1', 'home'])
            draw_odds = self._extract_odds(coeffs, ['kx', 'draw', 'coefficientX', 'x', 'X', 'tie'])
            away_odds = self._extract_odds(coeffs, ['k2', 'win2', 'w2', 'coefficient2', '2', 'away'])

            if home_odds < 1.01 and away_odds < 1.01:
                return None

            sport_raw = raw.get('sport', '').lower()
            sport = self.SPORT_MAP.get(sport_raw, 'football')

            league = (
                raw.get('champ')
                or raw.get('league')
                or raw.get('tournament')
                or raw.get('competition')
                or raw.get('leagueName')
                or 'Live' if is_live else 'Prematch'
            )

            event_id = (
                raw.get('id')
                or raw.get('eventId')
                or raw.get('gameId')
                or raw.get('matchId')
                or f"pari_{hash(home + away)}"
            )

            return {
                'id': f"pari_{event_id}",
                'bookmaker': 'pari',
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
            logger.debug(f"Pari: error normalizing event: {e}")
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
