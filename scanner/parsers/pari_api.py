# scanner/parsers/pari_api.py
"""
Pari parser - uses official REST API (shared platform with Bettery/Fonbet).
Endpoints:
  Live: /events/listBase?lang=ru&scopeMarket=2300
  Prematch: /events/listBase?lang=ru&scopeMarket=2300
Factor IDs: 921=П1, 922=X, 923=П2, 924=ТБ, 925=ТМ
"""
import asyncio
import time
import logging
from typing import List, Dict, Optional
import requests
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class PariParser(BaseParser):
    name = "Pari"
    slug = "pari"
    base_url = "https://pari.ru"
    
    # Pari API endpoints (shared platform)
    LIVE_API = "https://line-lb01-w.pb06e2-resources.com/events/list?lang=ru&scopeMarket=2300"
    PREMATCH_API = "https://line-lb01-w.pb06e2-resources.com/events/listBase?lang=ru&scopeMarket=2300"
    
    # Factor IDs for 1X2
    FACTOR_1 = 921
    FACTOR_X = 922
    FACTOR_2 = 923
    
    async def get_events(self) -> List[Dict]:
        events = []
        loop = asyncio.get_event_loop()
        
        # Try live first, then prematch
        for api_url, is_live in [(self.LIVE_API, True), (self.PREMATCH_API, False)]:
            try:
                data = await loop.run_in_executor(None, self._fetch_api_sync, api_url)
                if data:
                    parsed = self._parse_response(data, is_live=is_live)
                    events.extend(parsed)
                    logger.info(f"Pari: got {len(parsed)} events from {'live' if is_live else 'prematch'}")
            except Exception as e:
                logger.debug(f"Pari: failed to fetch {api_url}: {e}")
        
        logger.info(f"Pari: total {len(events)} events")
        return events
    
    def _parse_response(self, data: Dict, is_live: bool = True) -> List[Dict]:
        events = []
        if not isinstance(data, dict):
            return events
        
        matches = data.get('events', [])
        if not isinstance(matches, list):
            return events
        
        custom_factors = data.get('customFactors', [])
        if not isinstance(custom_factors, list):
            custom_factors = []
        
        # Build factor lookup: event_id -> {factor_id -> value}
        factor_map = {}
        for cf in custom_factors:
            if not isinstance(cf, dict):
                continue
            evt_id = cf.get('e')
            factors = cf.get('factors', [])
            if evt_id and isinstance(factors, list):
                factor_map[evt_id] = {f.get('f'): f.get('v') for f in factors if isinstance(f, dict)}
        
        for match in matches:
            if not isinstance(match, dict):
                continue
            
            home = str(match.get('team1', '')).strip()
            away = str(match.get('team2', '')).strip()
            
            if not home or not away:
                continue
            
            evt_id = match.get('id')
            factors = factor_map.get(evt_id, {})
            
            home_odds = float(factors.get(self.FACTOR_1, 0) or 0)
            draw_odds = float(factors.get(self.FACTOR_X, 0) or 0)
            away_odds = float(factors.get(self.FACTOR_2, 0) or 0)
            
            if home_odds > 1 or away_odds > 1:
                events.append({
                    'id': f"pari_{evt_id}",
                    'bookmaker': 'pari',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': match.get('name', 'Unknown'),
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1 else None,
                    'away_odds': away_odds,
                    'is_live': is_live,
                    'market': '1x2',
                    'source_url': f"{self.base_url}/{'live' if is_live else 'line'}/football",
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
            logger.debug(f"Pari API error: {e}")
        return None
