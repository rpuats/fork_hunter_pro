# scanner/parsers/zenit_parser.py
"""
Zenit API Parser - uses official REST API with proper imprinthash header.

CRITICAL: API requires 'imprinthash' header, without it returns:
  {"errorCode":400,"msg":"Не передан imprintHash в заголовках"}
Value d01d68e5a9775b90a0c7239e7f078895 captured from real browser.

JSON fields:
  name1  — home team
  name2  — away team
  k1     — home win odds (string "2.10")
  kx     — draw odds (string)
  k2     — away win odds (string)
  date   — match date "YYYY-MM-DD HH:MM:SS"
  tournament_name — league/tournament name
"""
import asyncio
import time
import logging
from typing import List, Dict, Optional
import requests
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class ZenitParser(BaseParser):
    name = "Zenit"
    slug = "zenit"
    base_url = "https://zenit.win"

    # Captured from real browser
    IMPRINT_HASH = "d01d68e5a9775b90a0c7239e7f078895"
    FRONT_VERSION = "1.72.1"

    # Sport IDs
    SPORT_FOOTBALL = 1
    SPORT_HOCKEY = 2
    SPORT_BASKETBALL = 3
    SPORT_TENNIS = 5

    # API URLs
    LINE_URL = "https://zenit.win/ajax/line/printer/react"
    LIVE_URL = "https://zenit.win/ajax/live/printer/react"

    async def get_events(self) -> List[Dict]:
        events = []
        loop = asyncio.get_event_loop()

        # Sports to fetch
        sports_to_fetch = [
            (self.SPORT_FOOTBALL, "football"),
            (self.SPORT_HOCKEY, "hockey"),
            (self.SPORT_BASKETBALL, "basketball"),
            (self.SPORT_TENNIS, "tennis"),
        ]

        for sport_id, sport_name in sports_to_fetch:
            # Fetch line (prematch) events
            try:
                line_events = await loop.run_in_executor(None, self._fetch_sport, self.LINE_URL, sport_id, False)
                events.extend(line_events)
                logger.info(f"Zenit: got {len(line_events)} {sport_name} prematch events")
            except Exception as e:
                logger.debug(f"Zenit: failed to fetch {sport_name} prematch: {e}")

            # Fetch live events
            try:
                live_events = await loop.run_in_executor(None, self._fetch_sport, self.LIVE_URL, sport_id, True)
                events.extend(live_events)
                logger.info(f"Zenit: got {len(live_events)} {sport_name} live events")
            except Exception as e:
                logger.debug(f"Zenit: failed to fetch {sport_name} live: {e}")

        logger.info(f"Zenit: total {len(events)} events collected")
        return events

    def _fetch_sport(self, base_url: str, sport_id: int, is_live: bool) -> List[Dict]:
        """Fetch events for one sport from one endpoint"""
        headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            "Accept": "application/json, text/javascript, */*; q=0.01",
            "Accept-Language": "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7",
            "Accept-Encoding": "gzip, deflate, br",
            "Referer": "https://zenit.win/line/football",
            "X-Requested-With": "XMLHttpRequest",
            # CRITICAL: without these headers API returns error 400
            "imprinthash": self.IMPRINT_HASH,
            "frontversion": self.FRONT_VERSION,
        }

        params = {
            "all": "1",
            "onlyview": "1",
            "timeline": "0",
            "tournaments_mode": "1",
            "sport": str(sport_id),
            "tournament": "",
            "tournament_region": "",
            "tournament_info": "",
            "league": "",
            "games": "",
            "ross": "0",
            "lang_id": "1",
            "timezone": "3",
            "offset": "0",
            "show_from_main": "0",
            "client_v": "",
            "length": "1000",
            "sort_mode": "2",
            "b_id": "",
            "popular": "0",
        }

        try:
            resp = requests.get(base_url, headers=headers, params=params, timeout=30)
            if resp.status_code == 200:
                data = resp.json()
                return self._parse_response(data, sport_id, is_live, base_url)
            else:
                logger.debug(f"Zenit API error: {resp.status_code}")
        except Exception as e:
            logger.debug(f"Zenit API request failed: {e}")

        return []

    def _parse_response(self, data: dict, sport_id: int, is_live: bool, source_url: str) -> List[Dict]:
        events = []

        # Check for application-level error
        if "errorCode" in data:
            msg = data.get("msg", "unknown error")
            logger.debug(f"Zenit API application error: {msg}")
            return events

        games_dict = data.get("games", {})
        if not isinstance(games_dict, dict):
            return events

        # Get dict section
        dict_section = data.get("dict", {})
        if not isinstance(dict_section, dict):
            return events

        # Get team names dictionary
        team_dict = dict_section.get("cmd", {})
        if not isinstance(team_dict, dict):
            team_dict = {}

        sport_map = {
            1: "football",
            2: "hockey",
            3: "basketball",
            5: "tennis",
        }
        sport = sport_map.get(sport_id, "football")

        for game_id, game in games_dict.items():
            if not isinstance(game, dict):
                continue

            # Get team IDs
            c1_id = game.get("c1_id")
            c2_id = game.get("c2_id")
            if not c1_id or not c2_id:
                continue

            # Get team names
            home = team_dict.get(str(c1_id), "").strip()
            away = team_dict.get(str(c2_id), "").strip()

            if not home or not away:
                continue

            # Get league name
            league = "Unknown"
            tid = game.get("tid")
            if tid:
                tournament_dict = dict_section.get("tournament", {})
                if isinstance(tournament_dict, dict):
                    league_info = tournament_dict.get(str(tid), {})
                    if isinstance(league_info, dict):
                        league = league_info.get("name", "Unknown")

            # Extract 1X2 odds from f_l or bets
            odds_data = game.get("f_l", []) or game.get("bets", [])
            if not isinstance(odds_data, list):
                continue

            home_odds = None
            draw_odds = None
            away_odds = None

            # Sports without draw: basketball (3), tennis (5)
            sports_without_draw = {3, 5}

            for bet in odds_data:
                if not isinstance(bet, dict):
                    continue

                bet_option = bet.get("o")
                bet_value = bet.get("h")

                if bet_option == "1" and bet_value:  # Home win
                    home_odds = self._parse_odds_str(str(bet_value))
                elif bet_option == "2" and bet_value and sport_id not in sports_without_draw:  # Away win (for sports with draw)
                    away_odds = self._parse_odds_str(str(bet_value))
                elif bet_option == "3" and bet_value:
                    if sport_id in sports_without_draw:
                        # For sports without draw, "3" is away
                        away_odds = self._parse_odds_str(str(bet_value))
                    else:
                        # For sports with draw, "3" is draw
                        draw_odds = self._parse_odds_str(str(bet_value))

            # Must have at least home and away odds
            if not home_odds or not away_odds:
                continue

            event = {
                'id': f"zenit_{game_id}",
                'bookmaker': 'zenit',
                'sport': sport,
                'home_team': home,
                'away_team': away,
                'league': league,
                'home_odds': home_odds,
                'draw_odds': draw_odds,
                'away_odds': away_odds,
                'is_live': is_live,
                'market': '1x2',
                'source_url': source_url,
                'scraped_at': time.time()
            }
            events.append(event)

        return events

    def _parse_odds_str(self, odds_str: Optional[str]) -> Optional[float]:
        """Parse odds string like '2.10' to float. Returns None if invalid or <= 1.0"""
        if not odds_str or not isinstance(odds_str, str):
            return None

        try:
            val = float(odds_str.strip())
            return val if val > 1.0 else None
        except (ValueError, AttributeError):
            return None
