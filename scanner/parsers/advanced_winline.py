# scanner/parsers/advanced_winline.py
"""
Advanced Winline Parser - Real data extraction
Uses multiple strategies: API, Playwright, fallback
"""
import asyncio
import aiohttp
import json
import re
import time
import logging
from typing import List, Dict, Optional
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class AdvancedWinlineParser(BaseParser):
    """
    Advanced parser for Winline with multiple extraction strategies.
    """
    name = "Winline Advanced"
    slug = "winline"
    base_url = "https://winline.ru"
    
    async def get_events(self) -> List[Dict]:
        """Get events using multiple strategies"""
        events = []
        
        # Strategy 1: Try internal API endpoints
        api_events = await self._try_api_endpoints()
        if api_events:
            events.extend(api_events)
            logger.info(f"Winline: Got {len(api_events)} events from API")
            return events[:50]
        
        # Strategy 2: Try WebSocket data
        ws_events = await self._try_websocket()
        if ws_events:
            events.extend(ws_events)
            logger.info(f"Winline: Got {len(ws_events)} events from WebSocket")
            return events[:50]
        
        # Strategy 3: Try HTML parsing
        html_events = await self._try_html_parsing()
        if html_events:
            events.extend(html_events)
            logger.info(f"Winline: Got {len(html_events)} events from HTML")
            return events[:50]
        
        logger.warning(f"Winline: All strategies failed")
        return events
    
    async def _try_api_endpoints(self) -> List[Dict]:
        """Try various API endpoints"""
        endpoints = [
            "https://winline.ru/api/content/line/football/live",
            "https://winline.ru/api/content/line/football/prematch",
            "https://winline.ru/api/v1/events/live",
            "https://winline.ru/api/v1/events",
            "https://winline.ru/api/line/events",
            "https://client-api.winline.ru/line/1",
            "https://client-api.winline.ru/line/live",
        ]
        
        headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Accept": "application/json, text/plain, */*",
            "Referer": "https://winline.ru/",
            "Origin": "https://winline.ru",
        }
        
        for url in endpoints:
            try:
                data = await self.fetch(url, headers=headers)
                if data and isinstance(data, dict):
                    events = self._parse_api_response(data)
                    if events:
                        return events
            except Exception as e:
                logger.debug(f"Winline API failed {url}: {e}")
                continue
        
        return []
    
    async def _try_websocket(self) -> List[Dict]:
        """Try to connect to WebSocket for live data"""
        try:
            import aiohttp
            
            ws_url = "wss://winline.ru/ws/live"
            headers = {
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
            }
            
            async with aiohttp.ClientSession() as session:
                async with session.ws_connect(ws_url, headers=headers, timeout=aiohttp.ClientTimeout(total=5)) as ws:
                    msg = await ws.receive_json(timeout=3)
                    return self._parse_api_response(msg)
                    
        except Exception as e:
            logger.debug(f"Winline WebSocket failed: {e}")
            return []
    
    async def _try_html_parsing(self) -> List[Dict]:
        """Try to parse events from HTML page"""
        try:
            import aiohttp
            
            headers = {
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            }
            
            async with aiohttp.ClientSession() as session:
                async with session.get(
                    f"{self.base_url}/live/football",
                    headers=headers,
                    timeout=aiohttp.ClientTimeout(total=10)
                ) as resp:
                    if resp.status == 200:
                        html = await resp.text()
                        return self._parse_html_events(html)
                        
        except Exception as e:
            logger.debug(f"Winline HTML parsing failed: {e}")
        
        return []
    
    def _parse_api_response(self, data: Dict) -> List[Dict]:
        """Parse events from API response"""
        events = []
        
        # Find events in nested structure
        items = []
        
        # Common patterns
        if 'events' in data:
            items = data['events']
        elif 'data' in data and isinstance(data['data'], dict):
            if 'events' in data['data']:
                items = data['data']['events']
            elif 'line' in data['data']:
                items = data['data']['line']
        elif 'matches' in data:
            items = data['matches']
        elif 'sports' in data:
            items = data['sports']
        
        # Handle nested structures
        if isinstance(items, list):
            for item in items:
                event = self._extract_event_from_item(item)
                if event:
                    events.append(event)
        elif isinstance(items, dict):
            for key, value in items.items():
                if isinstance(value, list):
                    for item in value:
                        event = self._extract_event_from_item(item)
                        if event:
                            events.append(event)
        
        return events
    
    def _extract_event_from_item(self, item: Dict) -> Optional[Dict]:
        """Extract event from nested item"""
        try:
            # Find teams
            home = None
            away = None
            
            if 'home' in item:
                home = item['home']
            elif 'homeTeam' in item:
                home = item['homeTeam']
            elif 'team1' in item:
                home = item['team1']
            
            if 'away' in item:
                away = item['away']
            elif 'awayTeam' in item:
                away = item['awayTeam']
            elif 'team2' in item:
                away = item['team2']
            
            if not home or not away:
                return None
            
            # Find odds
            home_odds = 0
            away_odds = 0
            draw_odds = 0
            
            # Check various odds locations
            if 'k1' in item:
                home_odds = float(item['k1'])
            elif 'homeOdds' in item:
                home_odds = float(item['homeOdds'])
            elif 'coef1' in item:
                home_odds = float(item['coef1'])
            
            if 'k2' in item:
                away_odds = float(item['k2'])
            elif 'awayOdds' in item:
                away_odds = float(item['awayOdds'])
            elif 'coef2' in item:
                away_odds = float(item['coef2'])
            
            if 'kx' in item:
                draw_odds = float(item['kx'])
            elif 'drawOdds' in item:
                draw_odds = float(item['drawOdds'])
            
            if home_odds < 1.01 and away_odds < 1.01:
                return None
            
            return {
                'id': f"winline_{item.get('id', hash(str(home) + str(away)))}",
                'bookmaker': 'winline',
                'sport': item.get('sport', 'football'),
                'home_team': str(home),
                'away_team': str(away),
                'league': item.get('champ') or item.get('league') or item.get('tournament', 'Live'),
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': True,
                'market': '1x2',
                'source_url': f"{self.base_url}/live",
                'scraped_at': time.time()
            }
            
        except Exception as e:
            logger.debug(f"Winline event extraction failed: {e}")
            return None
    
    def _parse_html_events(self, html: str) -> List[Dict]:
        """Parse events from HTML"""
        events = []
        
        # Look for JSON in script tags
        json_patterns = [
            r'window\.__INITIAL_STATE__\s*=\s*({.*?});',
            r'window\.__PRELOADED_STATE__\s*=\s*({.*?});',
            r'window\.__DATA__\s*=\s*({.*?});',
        ]
        
        for pattern in json_patterns:
            matches = re.findall(pattern, html, re.DOTALL)
            for match in matches:
                try:
                    data = json.loads(match)
                    parsed = self._parse_api_response(data)
                    if parsed:
                        return parsed
                except:
                    continue
        
        # Look for inline data
        data_pattern = r'data-event="([^"]+)"'
        event_matches = re.findall(data_pattern, html)
        
        for event_data in event_matches:
            try:
                data = json.loads(event_data)
                event = self._extract_event_from_item(data)
                if event:
                    events.append(event)
            except:
                continue
        
        return events
