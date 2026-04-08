# scanner/parsers/sportbet_api.py
"""
Sportbet parser - uses official REST API.
Returns ALL events including tennis/hockey (2-way markets).
"""
import asyncio
import time
import logging
from typing import List, Dict, Optional
import requests
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class SportbetApiParser(BaseParser):
    name = "Sportbet"
    slug = "sportbet_api"
    base_url = "https://sportbet.ru"
    
    LIVE_API = "https://sportbet.ru/sport/v1/fixtures-tree-live"
    PREMATCH_API = "https://sportbet.ru/sport/v1/rating-fixtures-tree?period=ALL"
    
    async def get_events(self) -> List[Dict]:
        events = []
        loop = asyncio.get_event_loop()
        
        for api_url, is_live in [(self.LIVE_API, True), (self.PREMATCH_API, False)]:
            try:
                data = await loop.run_in_executor(None, self._fetch_api_sync, api_url)
                if data:
                    parsed = self._parse_response(data, is_live=is_live)
                    events.extend(parsed)
                    logger.info(f"Sportbet: got {len(parsed)} events from {'live' if is_live else 'prematch'}")
            except Exception as e:
                logger.debug(f"Sportbet: failed to fetch {api_url}: {e}")
        
        logger.info(f"Sportbet: total {len(events)} events")
        return events
    
    def _parse_response(self, data: Dict, is_live: bool = True) -> List[Dict]:
        events = []
        if not isinstance(data, dict):
            return events
        
        fixtures = data.get('fixtures', {})
        markets = data.get('m', {})
        leagues = data.get('l', [])
        
        league_map = {item.get('i'): item.get('l', 'Unknown') for item in leagues if isinstance(item, dict)}
        
        for fix_id, fixture in fixtures.items():
            if not isinstance(fixture, dict):
                continue
            
            comps = fixture.get('c', [])
            if len(comps) < 2:
                continue
            
            home = str(comps[0].get('n', '')).strip()
            away = str(comps[1].get('n', '')).strip()
            
            if not home or not away:
                continue
            
            league_id = fixture.get('l')
            league = league_map.get(league_id, 'Unknown')
            sport_id = fixture.get('s')
            
            fixture_markets = markets.get(fix_id, [])
            if not isinstance(fixture_markets, list):
                continue
            
            home_odds = draw_odds = away_odds = 0.0
            
            for market in fixture_markets:
                if not isinstance(market, dict):
                    continue
                market_name = market.get('n', '').lower()
                
                # Find main 1x2 or winner market
                if market_name == '1x2':
                    outcomes = market.get('m', [])
                    if isinstance(outcomes, list) and len(outcomes) > 0:
                        for outcome in outcomes:
                            if not isinstance(outcome, dict):
                                continue
                            selections = outcome.get('sel', [])
                            for sel in selections:
                                if not isinstance(sel, dict):
                                    continue
                                name = sel.get('n', '')
                                odds = float(sel.get('o', 0) or 0)
                                
                                if name == home:
                                    home_odds = odds
                                elif 'ничья' in name.lower() or 'draw' in name.lower():
                                    draw_odds = odds
                                elif name == away:
                                    away_odds = odds
                # For tennis/hockey - use "Победитель" or "Winner" market
                elif 'победитель' in market_name or 'winner' in market_name:
                    outcomes = market.get('m', [])
                    if isinstance(outcomes, list) and len(outcomes) > 0:
                        for outcome in outcomes:
                            if not isinstance(outcome, dict):
                                continue
                            selections = outcome.get('sel', [])
                            for sel in selections:
                                if not isinstance(sel, dict):
                                    continue
                                name = sel.get('n', '')
                                odds = float(sel.get('o', 0) or 0)
                                
                                if name == home:
                                    home_odds = odds
                                elif name == away:
                                    away_odds = odds
            
            # Include events with at least one valid odd
            if home_odds > 1 or away_odds > 1:
                sport = 'football'
                if sport_id == 3:
                    sport = 'tennis'
                elif sport_id == 6:
                    sport = 'hockey'
                elif sport_id == 129:
                    sport = 'basketball'
                
                events.append({
                    'id': f"sportbet_{fix_id}",
                    'bookmaker': 'sportbet',
                    'sport': sport,
                    'home_team': home,
                    'away_team': away,
                    'league': league,
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1 else None,
                    'away_odds': away_odds,
                    'is_live': is_live,
                    'market': '1x2',
                    'source_url': f"{self.base_url}/{'live' if is_live else 'line'}/{sport}",
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
            logger.debug(f"Sportbet API error: {e}")
        return None
