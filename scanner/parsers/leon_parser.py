# scanner/parsers/leon_parser.py
"""
Leon parser - uses official REST API.
Full event endpoints (not just headline-matches which only returns ~50).
"""
import asyncio
import time
import logging
from typing import List, Dict, Optional
import requests
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class LeonParser(BaseParser):
    name = "Leon"
    slug = "leon"
    base_url = "https://leon.ru"
    
    # Full event APIs - returns ALL events (3000+)
    LIVE_API = "https://leon.ru/api-2/betline/events/inplay?ctag=ru-RU"
    PREMATCH_API = "https://leon.ru/api-2/betline/events/prematch?ctag=ru-RU"
    
    async def get_events(self) -> List[Dict]:
        events = []
        loop = asyncio.get_event_loop()
        
        for api_url, is_live in [(self.LIVE_API, True), (self.PREMATCH_API, False)]:
            try:
                data = await loop.run_in_executor(None, self._fetch_api_sync, api_url)
                if data:
                    parsed = self._parse_response(data, is_live=is_live)
                    events.extend(parsed)
                    logger.info(f"Leon: got {len(parsed)} events from {'live' if is_live else 'prematch'}")
            except Exception as e:
                logger.debug(f"Leon: failed to fetch {api_url}: {e}")
        
        logger.info(f"Leon: total {len(events)} events")
        return events
    
    def _parse_response(self, data: Dict, is_live: bool = True) -> List[Dict]:
        events = []
        if not isinstance(data, dict):
            return events
        
        matches = data.get('events', [])
        if not isinstance(matches, list):
            return events
        
        for match in matches:
            if not isinstance(match, dict):
                continue
            
            competitors = match.get('competitors', [])
            if len(competitors) < 2:
                continue
            
            home = str(competitors[0].get('name', '')).strip()
            away = str(competitors[1].get('name', '')).strip()
            
            if not home or not away:
                continue
            
            home_odds = draw_odds = away_odds = 0.0
            totals = []
            handicaps = []
            
            for market in match.get('markets', []):
                if not isinstance(market, dict):
                    continue
                runners = market.get('runners', [])
                market_name = market.get('name', '').lower()
                
                # Find 1X2 market - first market with 3 runners where first runner name is '1'
                if len(runners) == 3:
                    first_name = runners[0].get('name', '')
                    if first_name == '1':
                        for runner in runners:
                            if not isinstance(runner, dict):
                                continue
                            price = float(runner.get('price', 0) or 0)
                            runner_name = runner.get('name', '')
                            if runner_name == '1':
                                home_odds = price
                            elif runner_name == 'X':
                                draw_odds = price
                            elif runner_name == '2':
                                away_odds = price
                elif 'тотал' in market_name or 'total' in market_name:
                    for runner in runners:
                        if isinstance(runner, dict):
                            totals.append({
                                'type': runner.get('name', '').lower(),
                                'value': runner.get('handicap'),
                                'odd': float(runner.get('price', 0) or 0)
                            })
                elif 'фора' in market_name or 'handicap' in market_name:
                    for runner in runners:
                        if isinstance(runner, dict):
                            handicaps.append({
                                'type': runner.get('name', '').lower(),
                                'value': runner.get('handicap'),
                                'odd': float(runner.get('price', 0) or 0)
                            })
            
            if home_odds > 1 or away_odds > 1:
                league = match.get('league', {})
                league_name = league.get('name', 'Unknown') if isinstance(league, dict) else 'Unknown'
                events.append({
                    'id': f"leon_{match.get('id', hash(home + away))}",
                    'bookmaker': 'leon',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': league_name,
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1 else None,
                    'away_odds': away_odds,
                    'totals': totals,
                    'handicaps': handicaps,
                    'is_live': is_live,
                    'market': '1x2',
                    'source_url': f"{self.base_url}/{'live' if is_live else 'prematch'}/football",
                    'scraped_at': time.time()
                })
        
        return events
    
    def _fetch_api_sync(self, url: str) -> Optional[Dict]:
        headers = {
            "Accept": "application/json",
            "Referer": self.base_url,
            "Origin": self.base_url,
        }
        try:
            resp = requests.get(url, headers=headers, timeout=30)
            if resp.status_code == 200:
                return resp.json()
        except Exception as e:
            logger.debug(f"Leon API error: {e}")
        return None
