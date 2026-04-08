# scanner/parsers/olimp_parser.py
"""
OlimpBet parser - uses official REST API v4.
Structure: payload -> competitionsWithEvents[] -> events[] -> outcomes[]
Outcomes have: shortName ('П1','Х','П2'), probability (odds), marketId
"""
import asyncio
import time
import logging
from typing import List, Dict, Optional
import requests
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class OlimpParser(BaseParser):
    name = "Olimp"
    slug = "olimp"
    base_url = "https://www.olimp.bet"
    
    LIVE_API = "https://www.olimp.bet/api/v4/0/live/sports-with-competitions-with-events?vids%5B%5D="
    LINE_API = "https://www.olimp.bet/api/v4/0/line/top/sports-with-competitions-with-events?vids%5B%5D="
    
    async def get_events(self) -> List[Dict]:
        events = []
        loop = asyncio.get_event_loop()
        
        for api_url, is_live in [(self.LIVE_API, True), (self.LINE_API, False)]:
            try:
                data = await loop.run_in_executor(None, self._fetch_api_sync, api_url)
                if data:
                    parsed = self._parse_response(data, is_live=is_live)
                    events.extend(parsed)
                    logger.info(f"Olimp: got {len(parsed)} events from {'live' if is_live else 'prematch'}")
            except Exception as e:
                logger.debug(f"Olimp: failed to fetch {api_url}: {e}")
        
        logger.info(f"Olimp: total {len(events)} events")
        return events
    
    def _parse_response(self, data: List, is_live: bool = True) -> List[Dict]:
        events = []
        if not isinstance(data, list):
            return events
        
        for item in data:
            if not isinstance(item, dict):
                continue
            payload = item.get('payload')
            if not isinstance(payload, dict):
                continue
            
            sport = payload.get('sport', {})
            sport_name = sport.get('name', 'Unknown')
            
            comps = payload.get('competitionsWithEvents', [])
            if not isinstance(comps, list):
                continue
            
            for comp in comps:
                if not isinstance(comp, dict):
                    continue
                league_name = comp.get('name', sport_name)
                evts = comp.get('events', [])
                if not isinstance(evts, list):
                    continue
                
                for evt in evts:
                    if not isinstance(evt, dict):
                        continue
                    
                    home = str(evt.get('team1Name', '') or evt.get('name1', '')).strip()
                    away = str(evt.get('team2Name', '') or evt.get('name2', '')).strip()
                    
                    if not home or not away:
                        continue
                    
                    home_odds = draw_odds = away_odds = 0.0
                    totals = []
                    handicaps = []
                    
                    outcomes = evt.get('outcomes', [])
                    if isinstance(outcomes, list):
                        for o in outcomes:
                            if not isinstance(o, dict):
                                continue
                            short_name = o.get('shortName', '')
                            prob = o.get('probability', '0')
                            try:
                                odds = float(prob)
                            except (ValueError, TypeError):
                                odds = 0.0
                            market_id = o.get('marketId', 0)
                            handicap = o.get('param')
                            
                            # 1X2 market (marketId=1 for football/hockey, marketId=2 for tennis)
                            if market_id in [1, 2]:
                                if short_name == 'П1':
                                    home_odds = odds
                                elif short_name == 'Х':
                                    draw_odds = odds
                                elif short_name == 'П2':
                                    away_odds = odds
                            # Totals
                            elif 'тотал' in o.get('groupName', '').lower() or 'total' in o.get('groupName', '').lower():
                                totals.append({
                                    'type': short_name.lower(),
                                    'value': handicap,
                                    'odd': odds
                                })
                            # Handicaps
                            elif 'фора' in o.get('groupName', '').lower() or 'handicap' in o.get('groupName', '').lower():
                                handicaps.append({
                                    'type': short_name.lower(),
                                    'value': handicap,
                                    'odd': odds
                                })
                    
                    if home_odds > 1 or away_odds > 1:
                        events.append({
                            'id': f"olimp_{evt.get('id', hash(home + away))}",
                            'bookmaker': 'olimp',
                            'sport': 'football' if 'футбол' in sport_name.lower() else sport_name,
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
                            'source_url': f"{self.base_url}/{'live' if is_live else 'line'}/football",
                            'scraped_at': time.time()
                        })
        
        return events
    
    def _fetch_api_sync(self, url: str) -> Optional[List]:
        headers = {
            "Accept": "application/json",
            "Referer": self.base_url,
            "Origin": self.base_url,
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        }
        try:
            resp = requests.get(url, headers=headers, timeout=30)
            if resp.status_code == 200:
                return resp.json()
        except Exception as e:
            logger.debug(f"Olimp API error: {e}")
        return None
