# scanner/parsers/winline_parser.py
import logging
from typing import List, Dict, Optional
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class WinlineParser(BaseParser):
    name = "Winline"
    slug = "winline"
    base_url = "https://winline.ru"
    
    async def get_events(self) -> List[Dict]:
        events = []
        
        urls_to_try = [
            "https://winline.ru/live/football",
            "https://client-api.winline.ru/feed/1",
        ]
        
        for url in urls_to_try:
            try:
                data = await self.fetch(url)
                if data and isinstance(data, dict):
                    parsed = self._parse_response(data)
                    if parsed:
                        events.extend(parsed)
                        logger.info(f"Winline: got {len(parsed)} events from {url}")
                        break
            except Exception as e:
                logger.debug(f"Winline: failed to fetch {url}: {e}")
                continue
        
        logger.debug(f"Winline: total {len(events)} events collected")
        return events[:50]
    
    def _parse_response(self, data: Dict) -> List[Dict]:
        events = []
        
        items = data.get('events') or data.get('data', {}).get('events', [])
        
        for item in items if isinstance(items, list) else []:
            try:
                e = self._normalize_event(item)
                if e:
                    events.append(e)
            except Exception as e:
                logger.debug(f"Winline: failed to normalize event: {e}")
                continue
        
        return events
    
    def _normalize_event(self, raw: Dict) -> Optional[Dict]:
        try:
            teams = raw.get('teams', [])
            home = teams[0] if len(teams) > 0 else raw.get('team1', 'Home')
            away = teams[1] if len(teams) > 1 else raw.get('team2', 'Away')
            
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
                'id': f"winline_{raw.get('id', hash(home + away))}",
                'bookmaker': 'winline',
                'sport': raw.get('sport', 'football'),
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
            logger.debug(f"Winline: error normalizing event: {e}")
            return None
