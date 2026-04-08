# scanner/parsers/pinnacle_parser.py
"""
Pinnacle API parser - async parser for live odds.
Endpoint: https://api.pinnacle.com/v1/odds?sportId=29 (soccer)
"""
import asyncio
import time
import logging
from typing import List, Dict, Optional
import aiohttp
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class PinnacleParser(BaseParser):
    name = "Pinnacle"
    slug = "pinnacle"
    base_url = "https://api.pinnacle.com"

    ODDS_API = "https://api.pinnacle.com/v1/odds"
    SPORT_ID = 29

    async def get_events(self) -> List[Dict]:
        events = []

        params = {
            "sportId": self.SPORT_ID,
            "isLive": True,
        }

        try:
            url = f"{self.ODDS_API}?sportId={self.SPORT_ID}&isLive=true"
            data = await self._fetch_api(url)
            if data:
                parsed = self._parse_response(data, is_live=True)
                events.extend(parsed)
                logger.info(f"Pinnacle: got {len(parsed)} live events")
        except Exception as e:
            logger.error(f"Pinnacle: failed to fetch live events: {e}")

        try:
            url = f"{self.ODDS_API}?sportId={self.SPORT_ID}&isLive=false"
            data = await self._fetch_api(url)
            if data:
                parsed = self._parse_response(data, is_live=False)
                events.extend(parsed)
                logger.info(f"Pinnacle: got {len(parsed)} prematch events")
        except Exception as e:
            logger.debug(f"Pinnacle: failed to fetch prematch events: {e}")

        return events[:100]

    async def _fetch_api(self, url: str) -> Optional[Dict]:
        headers = {
            "Accept": "application/json",
            "Referer": "https://www.pinnacle.com",
            "Origin": "https://www.pinnacle.com",
        }

        try:
            data = await self.fetch(url, headers=headers)
            return data
        except Exception as e:
            logger.error(f"Pinnacle API error: {e}")
        return None

    def _parse_response(self, data: Dict, is_live: bool = True) -> List[Dict]:
        events = []

        if not isinstance(data, dict):
            return events

        leagues = data.get('league', []) or data.get('leagues', [])

        for league in leagues if isinstance(leagues, list) else []:
            league_name = league.get('name', 'Unknown League')
            events_list = league.get('events', [])

            for event in events_list if isinstance(events_list, list) else []:
                try:
                    event['league'] = league_name
                    event['isLive'] = is_live
                    normalized = self._normalize_event(event)
                    if normalized:
                        events.append(normalized)
                except Exception as e:
                    logger.debug(f"Pinnacle: failed to normalize event: {e}")

        return events

    def _normalize_event(self, raw: Dict) -> Optional[Dict]:
        try:
            home = raw.get('home') or raw.get('homeTeam') or raw.get('team1') or ''
            away = raw.get('away') or raw.get('awayTeam') or raw.get('team2') or ''

            home = str(home).strip()
            away = str(away).strip()

            if not home or not away:
                return None

            home_odds = 0.0
            draw_odds = 0.0
            away_odds = 0.0

            periods = raw.get('periods', [])
            for period in periods if isinstance(periods, list) else []:
                if not isinstance(period, dict):
                    continue

                line_id = period.get('lineId', 0)
                if line_id == 0:
                    continue

                moneyline = period.get('moneyline', {})
                if isinstance(moneyline, dict):
                    home_odds = float(moneyline.get('home', moneyline.get('homePrice', 0)))
                    draw_odds = float(moneyline.get('draw', moneyline.get('drawPrice', 0)))
                    away_odds = float(moneyline.get('away', moneyline.get('awayPrice', 0)))

                if home_odds > 1.0 or away_odds > 1.0:
                    break

            if home_odds < 1.01 and away_odds < 1.01:
                return None

            event_id = raw.get('id', raw.get('eventId', hash(home + away)))

            return {
                'id': f"pinnacle_{event_id}",
                'bookmaker': 'pinnacle',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': raw.get('league', 'Unknown'),
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': raw.get('isLive', True),
                'market': '1x2',
                'source_url': f"https://www.pinnacle.com/football/odds/{event_id}",
                'scraped_at': time.time()
            }
        except Exception as e:
            logger.debug(f"Pinnacle: error normalizing event: {e}")
            return None
