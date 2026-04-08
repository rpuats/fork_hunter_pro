# scanner/parsers/olimpbet_parser.py
import logging
from typing import List, Dict, Optional
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class OlimpBetParser(BaseParser):
    name = "Olimpbet"
    slug = "olimpbet"
    base_url = "https://olimp.bet"
    
    async def get_events(self) -> List[Dict]:
        events = []
        
        urls = [
            "https://olimp.bet/live/feed/1",
            "https://olimp.bet/api/live",
        ]
        
        for url in urls:
            try:
                data = await self.fetch(url)
                if data:
                    parsed = self._parse_response(data)
                    if parsed:
                        events.extend(parsed)
                        logger.info(f"Olimpbet: got {len(parsed)} events from {url}")
                        break
            except Exception as e:
                logger.debug(f"Olimpbet: failed to fetch {url}: {e}")
                continue
        
        logger.debug(f"Olimpbet: total {len(events)} events collected")
        return events[:50]
    
    def _parse_response(self, data: dict) -> List[Dict]:
        events = []
        
        items = data.get('data', []) or data.get('events', [])
        
        for item in items if isinstance(items, list) else []:
            try:
                e = self._normalize_event(item)
                if e:
                    events.append(e)
            except Exception as e:
                logger.debug(f"Olimpbet: failed to normalize event: {e}")
                continue
        
        return events
    
    def _normalize_event(self, raw: Dict) -> Optional[Dict]:
        try:
            home = raw.get('O1') or raw.get('team1', 'Home')
            away = raw.get('O2') or raw.get('team2', 'Away')
            
            home = str(home).strip()
            away = str(away).strip()
            
            if not home or not away:
                return None
            
            home_odds = float(raw.get('C1', raw.get('k1', 0)))
            draw_odds = float(raw.get('CX', raw.get('kx', 0)))
            away_odds = float(raw.get('C2', raw.get('k2', 0)))
            
            if home_odds < 1.01 and away_odds < 1.01:
                return None
            
            return {
                'id': f"olimpbet_{raw.get('I', hash(home + away))}",
                'bookmaker': 'olimpbet',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': raw.get('LN', 'Live'),
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': True,
                'market': '1x2',
                'source_url': f"{self.base_url}/live"
            }
        except Exception as e:
            logger.debug(f"Olimpbet: error normalizing event: {e}")
            return None
