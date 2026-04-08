# scanner/parsers/betcity_parser.py
import logging
from typing import List, Dict, Optional
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class BetcityParser(BaseParser):
    name = "Betcity"
    slug = "betcity"
    base_url = "https://betcity.ru"
    
    async def get_events(self) -> List[Dict]:
        events = []
        
        urls = [
            "https://betcity.ru/api/live",
            "https://betcity.ru/live",
        ]
        
        for url in urls:
            try:
                data = await self.fetch(url)
                if data:
                    parsed = self._parse_response(data)
                    if parsed:
                        events.extend(parsed)
                        logger.info(f"Betcity: got {len(parsed)} events from {url}")
                        break
            except Exception as e:
                logger.debug(f"Betcity: failed to fetch {url}: {e}")
                continue
        
        logger.debug(f"Betcity: total {len(events)} events collected")
        return events[:50]
    
    def _parse_response(self, data: dict) -> List[Dict]:
        events = []
        
        items = data.get('events', []) or data.get('games', []) or data.get('data', [])
        
        for item in items if isinstance(items, list) else []:
            try:
                e = self._normalize_event(item)
                if e:
                    events.append(e)
            except Exception as e:
                logger.debug(f"Betcity: failed to normalize event: {e}")
                continue
        
        return events
    
    def _normalize_event(self, raw: Dict) -> Optional[Dict]:
        try:
            home = raw.get('team1') or raw.get('home', 'Home')
            away = raw.get('team2') or raw.get('away', 'Away')
            
            home = str(home).strip()
            away = str(away).strip()
            
            if not home or not away:
                return None
            
            home_odds = float(raw.get('k1') or raw.get('win1') or raw.get('coefficient1', 0))
            draw_odds = float(raw.get('kx') or raw.get('draw') or raw.get('coefficientX', 0))
            away_odds = float(raw.get('k2') or raw.get('win2') or raw.get('coefficient2', 0))
            
            if home_odds < 1.01 and away_odds < 1.01:
                return None
            
            return {
                'id': f"betcity_{raw.get('id', hash(home + away))}",
                'bookmaker': 'betcity',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': raw.get('champ') or raw.get('league', 'Live'),
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': True,
                'market': '1x2',
                'source_url': f"{self.base_url}/live"
            }
        except Exception as e:
            logger.debug(f"Betcity: error normalizing event: {e}")
            return None
