# scanner/parsers/leon_api.py
"""
Leon parser - uses official API endpoint discovered via exploration.
Endpoint: https://leon.bet/api-2/betline/events/inplayupcomingall?ctag=ru-RU&hideClosed=tr
"""
import asyncio
import time
import logging
from typing import List, Dict, Optional
import aiohttp
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class LeonParser(BaseParser):
    name = "Leon"
    slug = "leon"
    base_url = "https://leon.ru"
    
    LIVE_API = "https://leon.ru/api-2/betline/headline-matches?ctag=ru-RU&flags=reg,urlv2,orn2,mm2,rrc&merged=true"
    PREMATCH_API = "https://leon.ru/api-2/betline/headline-matches?ctag=ru-RU&flags=reg,urlv2,orn2,mm2,rrc&merged=true"
    
    async def get_events(self) -> List[Dict]:
        events = []
        
        url = self.LIVE_API
        try:
            data = await self._fetch_api(url)
            if data:
                # Parse the headline-matches structure
                parsed = self._parse_headline_matches(data)
                events.extend(parsed)
                logger.info(f"Leon: got {len(parsed)} events")
        except Exception as e:
            logger.debug(f"Leon: failed to fetch {url}: {e}")
        
        return events
    
    def _parse_headline_matches(self, data: Dict) -> List[Dict]:
        """Parse Leon headline-matches API response"""
        events = []
        
        if not isinstance(data, dict):
            return events
        
        # Navigate: data -> events -> events[]
        events_container = data.get('events', {})
        if not isinstance(events_container, dict):
            return events
        
        matches = events_container.get('events', [])
        if not isinstance(matches, list):
            return []
        
        for match in matches:
            if not isinstance(match, dict):
                continue
            
            # Check if match is open
            if not match.get('open', False):
                continue
            
            # Get competitors
            competitors = match.get('competitors', [])
            if len(competitors) < 2:
                continue
            
            home = str(competitors[0].get('name', '')).strip()
            away = str(competitors[1].get('name', '')).strip()
            
            if not home or not away:
                continue
            
            # Get markets - find 1X2 market (first market with 3 runners)
            markets = match.get('markets', [])
            home_odds = 0.0
            draw_odds = 0.0
            away_odds = 0.0
            
            for market in markets:
                if not isinstance(market, dict):
                    continue
                runners = market.get('runners', [])
                
                # 1X2 market has exactly 3 runners
                if len(runners) == 3:
                    for i, runner in enumerate(runners):
                        if not isinstance(runner, dict):
                            continue
                        price = float(runner.get('price', 0) or 0)
                        if i == 0:
                            home_odds = price
                        elif i == 1:
                            draw_odds = price
                        elif i == 2:
                            away_odds = price
                    break  # Found 1X2 market
            
            if home_odds > 1 or away_odds > 1:
                is_live = match.get('betline') == 'inplay'
                events.append({
                    'id': f"leon_{match.get('id', hash(home + away))}",
                    'bookmaker': 'leon',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': 'Live' if is_live else 'Pre-match',
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1 else None,
                    'away_odds': away_odds,
                    'is_live': is_live,
                    'market': '1x2',
                    'source_url': 'https://leon.ru/live/football',
                    'scraped_at': time.time()
                })
        
        return events
        
        # Navigate: data -> events -> events[]
        events_container = data.get('events', {})
        if not isinstance(events_container, dict):
            return events
        
        matches = events_container.get('events', [])
        if not isinstance(matches, list):
            return events
        
        for match in matches:
            if not isinstance(match, dict):
                continue
            
            # Get competitors
            competitors = match.get('competitors', [])
            if len(competitors) < 2:
                continue
            
            home = competitors[0].get('name', '')
            away = competitors[1].get('name', '')
            
            if not home or not away:
                continue
            
            # Get markets
            markets = match.get('markets', [])
            home_odds = 0.0
            draw_odds = 0.0
            away_odds = 0.0
            
            for market in markets:
                if not isinstance(market, dict):
                    continue
                market_id = market.get('id', 0)
                runners = market.get('runners', [])
                
                # Market ID 1 = 1X2
                if market_id == 1 and len(runners) >= 3:
                    for runner in runners:
                        if not isinstance(runner, dict):
                            continue
                        runner_id = runner.get('id', 0)
                        price = runner.get('price', 0)
                        if runner_id == 1:
                            home_odds = price
                        elif runner_id == 2:
                            draw_odds = price
                        elif runner_id == 3:
                            away_odds = price
            
            if home_odds > 1 or away_odds > 1:
                is_live = match.get('betline') == 'inplay'
                events.append({
                    'id': f"leon_{match.get('id', hash(home + away))}",
                    'bookmaker': 'leon',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': 'Live' if is_live else 'Pre-match',
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1 else None,
                    'away_odds': away_odds,
                    'is_live': is_live,
                    'market': '1x2',
                    'source_url': 'https://leon.ru/live/football',
                    'scraped_at': time.time()
                })
        
        return events[:50]
    
    async def _fetch_api(self, url: str) -> Optional[Dict]:
        headers = {
            "Accept": "application/json",
            "Referer": self.base_url,
            "Origin": self.base_url,
        }
        
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(url, headers=headers, timeout=aiohttp.ClientTimeout(total=15)) as resp:
                    if resp.status == 200:
                        return await resp.json()
        except Exception as e:
            logger.debug(f"Leon API error: {e}")
        return None
    
    def _parse_response(self, data: Dict, is_live: bool = True) -> List[Dict]:
        events = []
        
        if not isinstance(data, dict):
            return events
        
        items = data.get('events', []) or data.get('data', []) or []
        
        if isinstance(items, dict):
            items = list(items.values())
        
        for item in items if isinstance(items, list) else []:
            try:
                event = self._normalize_event(item, is_live=is_live)
                if event:
                    events.append(event)
            except Exception as e:
                logger.debug(f"Leon: failed to normalize event: {e}")
        
        return events
    
    def _normalize_event(self, raw: Dict, is_live: bool = True) -> Optional[Dict]:
        try:
            # Get competitors
            competitors = raw.get('competitors', [])
            if len(competitors) < 2:
                return None
            
            home = competitors[0].get('name', '')
            away = competitors[1].get('name', '')
            
            home = str(home).strip()
            away = str(away).strip()
            
            if not home or not away:
                return None
            
            # Get markets
            markets = raw.get('markets', [])
            home_odds = 0.0
            draw_odds = 0.0
            away_odds = 0.0
            
            for market in markets:
                if not isinstance(market, dict):
                    continue
                market_id = market.get('id', 0)
                runners = market.get('runners', [])
                
                # Market ID 1 = 1X2
                if market_id == 1 and len(runners) >= 3:
                    for runner in runners:
                        if not isinstance(runner, dict):
                            continue
                        runner_id = runner.get('id', 0)
                        price = runner.get('price', 0)
                        if runner_id == 1:
                            home_odds = price
                        elif runner_id == 2:
                            draw_odds = price
                        elif runner_id == 3:
                            away_odds = price
            
            if home_odds < 1.01 and away_odds < 1.01:
                return None
            
            is_live = raw.get('betline') == 'inplay'
            
            return {
                'id': f"leon_{raw.get('id', hash(home + away))}",
                'bookmaker': 'leon',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': 'Live' if is_live else 'Pre-match',
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': is_live,
                'market': '1x2',
                'source_url': f"{self.base_url}/{'live' if is_live else 'prematch'}",
                'scraped_at': time.time()
            }
        except Exception as e:
            logger.debug(f"Leon: error normalizing event: {e}")
            return None
