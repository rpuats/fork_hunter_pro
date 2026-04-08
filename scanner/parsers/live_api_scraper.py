# scanner/parsers/live_api_scraper.py
import asyncio
import aiohttp
from typing import List, Dict, Optional
from datetime import datetime
import json
import re


class LiveAPIScraper:
    """Универсальный сканер live-событий через API букмекеров"""
    
    def __init__(self):
        self.session: Optional[aiohttp.ClientSession] = None
        self.headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Accept": "application/json, text/plain, */*",
            "Accept-Language": "ru-RU,ru;q=0.9,en;q=0.8",
            "Origin": "",
            "Referer": ""
        }
    
    async def get_session(self) -> aiohttp.ClientSession:
        if self.session is None or self.session.closed:
            timeout = aiohttp.ClientTimeout(total=30)
            self.session = aiohttp.ClientSession(headers=self.headers, timeout=timeout)
        return self.session
    
    async def close(self):
        if self.session and not self.session.closed:
            await self.session.close()
    
    async def fetch_json(self, url: str, headers: Dict = None) -> Optional[Dict]:
        try:
            session = await self.get_session()
            async with session.get(url, headers=headers or {}, timeout=aiohttp.ClientTimeout(total=30)) as resp:
                if resp.status == 200:
                    return await resp.json()
                return None
        except Exception:
            return None
    
    async def parse_winline(self) -> List[Dict]:
        """Winline API парсинг"""
        events = []
        
        apis = [
            "https://winline声响.github.io/api/live.json",
            "https://api.winline.ru/cityline/live",
        ]
        
        for api_url in apis:
            try:
                session = await self.get_session()
                async with session.get(api_url, timeout=aiohttp.ClientTimeout(total=15)) as resp:
                    if resp.status == 200:
                        data = await resp.json()
                        for item in self._extract_winline_events(data):
                            item['bookmaker'] = 'winline'
                            events.append(item)
                        if events:
                            break
            except:
                continue
        
        return events[:50]
    
    def _extract_winline_events(self, data: Dict) -> List[Dict]:
        events = []
        
        def extract_recursive(obj, path=""):
            if isinstance(obj, dict):
                if 'events' in obj and isinstance(obj['events'], list):
                    for event in obj['events']:
                        e = self._parse_winline_event(event)
                        if e:
                            events.append(e)
                
                for key, value in obj.items():
                    extract_recursive(value, f"{path}.{key}")
            
            elif isinstance(obj, list):
                for i, item in enumerate(obj):
                    extract_recursive(item, f"{path}[{i}]")
        
        extract_recursive(data)
        return events
    
    def _parse_winline_event(self, raw: Dict) -> Optional[Dict]:
        try:
            teams = raw.get('teams') or raw.get('name', '').split(' - ')
            if len(teams) >= 2:
                home = teams[0]
                away = teams[1]
            else:
                home = raw.get('team1', raw.get('home', 'Unknown'))
                away = raw.get('team2', raw.get('away', 'Unknown'))
            
            home_odds = float(raw.get('k1') or raw.get('win1') or raw.get('coefficient1') or 0)
            draw_odds = float(raw.get('kx') or raw.get('draw') or raw.get('coefficientX') or 0)
            away_odds = float(raw.get('k2') or raw.get('win2') or raw.get('coefficient2') or 0)
            
            if home_odds > 1.01 or away_odds > 1.01:
                return {
                    'id': raw.get('id', str(hash(home + away))),
                    'bookmaker': 'winline',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': raw.get('champ') or raw.get('league') or 'Live',
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1.0 else None,
                    'away_odds': away_odds,
                    'is_live': True,
                    'market': '1x2',
                    'start_time': raw.get('start', raw.get('time', ''))
                }
        except:
            pass
        return None
    
    async def parse_olimp(self) -> List[Dict]:
        """Olimp API парсинг"""
        events = []
        
        try:
            session = await self.get_session()
            headers = {
                "User-Agent": "Mozilla/5.0",
                "Accept": "application/json",
                "X-Requested-With": "XMLHttpRequest"
            }
            
            url = "https://www.olimp.bet/live/feed/1"
            async with session.get(url, headers=headers, timeout=aiohttp.ClientTimeout(total=15)) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    for item in self._extract_olimp_events(data):
                        item['bookmaker'] = 'olimp'
                        events.append(item)
        except:
            pass
        
        return events[:50]
    
    def _extract_olimp_events(self, data: Dict) -> List[Dict]:
        events = []
        
        if isinstance(data, dict):
            items = data.get('data') or data.get('events') or data.get('Value', [])
            if isinstance(items, list):
                for item in items:
                    e = self._parse_olimp_event(item)
                    if e:
                        events.append(e)
        
        return events
    
    def _parse_olimp_event(self, raw: Dict) -> Optional[Dict]:
        try:
            home = raw.get('O1') or raw.get('team1', 'Home')
            away = raw.get('O2') or raw.get('team2', 'Away')
            
            home_odds = float(raw.get('C1', raw.get('k1', 0)))
            draw_odds = float(raw.get('CX', raw.get('kx', 0)))
            away_odds = float(raw.get('C2', raw.get('k2', 0)))
            
            if home_odds > 1.01 or away_odds > 1.01:
                return {
                    'id': raw.get('I', str(hash(home + away))),
                    'bookmaker': 'olimp',
                    'sport': 'football',
                    'home_team': home,
                    'away_team': away,
                    'league': raw.get('LN', 'Live'),
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1.0 else None,
                    'away_odds': away_odds,
                    'is_live': True,
                    'market': '1x2'
                }
        except:
            pass
        return None
    
    async def parse_pari(self) -> List[Dict]:
        """Pari API парсинг"""
        events = []
        
        try:
            session = await self.get_session()
            
            url = "https://www.pari.ru/LiveFeed/GetGamesHtml"
            params = {
                'gamezone': 'live',
                'sports': '1',
                'count': '50',
                'mode': '4'
            }
            
            async with session.get(url, params=params, timeout=aiohttp.ClientTimeout(total=15)) as resp:
                if resp.status == 200:
                    text = await resp.text()
                    events.extend(self._parse_pari_html(text))
        except:
            pass
        
        return events[:50]
    
    def _parse_pari_html(self, html: str) -> List[Dict]:
        events = []
        
        patterns = [
            r'"team1":"([^"]+)".*?"team2":"([^"]+)".*?"k1":([0-9.]+).*?"k2":([0-9.]+)',
            r'data-team1="([^"]+)".*?data-team2="([^"]+)".*?data-k1="([0-9.]+)".*?data-k2="([0-9.]+)"',
        ]
        
        for pattern in patterns:
            matches = re.findall(pattern, html.replace('\\"', '"'))
            for match in matches:
                if len(match) >= 4:
                    home, away, k1, k2 = match[:4]
                    k1_f = float(k1) if k1 else 0
                    k2_f = float(k2) if k2 else 0
                    
                    if k1_f > 1.01 or k2_f > 1.01:
                        events.append({
                            'id': str(hash(home + away)),
                            'bookmaker': 'pari',
                            'sport': 'football',
                            'home_team': home,
                            'away_team': away,
                            'league': 'Live',
                            'home_odds': k1_f,
                            'away_odds': k2_f,
                            'is_live': True,
                            'market': '1x2'
                        })
        
        return events
    
    async def parse_fonbet(self) -> List[Dict]:
        """Fonbet API парсинг"""
        events = []
        
        try:
            session = await self.get_session()
            
            url = "https://client-api.24h.bet/api/v2/client/line/live/football"
            headers = {
                "User-Agent": "Mozilla/5.0",
                "Accept": "application/json"
            }
            
            async with session.get(url, headers=headers, timeout=aiohttp.ClientTimeout(total=15)) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    for item in self._extract_fonbet_events(data):
                        item['bookmaker'] = 'fonbet'
                        events.append(item)
        except:
            pass
        
        return events[:50]
    
    def _extract_fonbet_events(self, data: Dict) -> List[Dict]:
        events = []
        
        if isinstance(data, dict):
            items = data.get('events') or data.get('games') or []
            
        for item in items if isinstance(items, list) else []:
            try:
                home = item.get('name', '').split(' - ')[0] if ' - ' in item.get('name', '') else item.get('team1', 'Home')
                away = item.get('name', '').split(' - ')[-1] if ' - ' in item.get('name', '') else item.get('team2', 'Away')
                
                coeffs = item.get('coeffs', {})
                home_odds = float(coeffs.get('w1') or coeffs.get('win1') or coeffs.get('1') or 0)
                away_odds = float(coeffs.get('w2') or coeffs.get('win2') or coeffs.get('2') or 0)
                draw_odds = float(coeffs.get('draw') or coeffs.get('x') or coeffs.get('X') or 0)
                
                if home_odds > 1.01 or away_odds > 1.01:
                    events.append({
                        'id': str(item.get('id', hash(home + away))),
                        'bookmaker': 'fonbet',
                        'sport': 'football',
                        'home_team': home,
                        'away_team': away,
                        'league': item.get('category', 'Live'),
                        'home_odds': home_odds,
                        'draw_odds': draw_odds if draw_odds > 1.0 else None,
                        'away_odds': away_odds,
                        'is_live': True,
                        'market': '1x2'
                    })
            except:
                continue
        
        return events
    
    async def parse_all(self) -> List[Dict]:
        """Парсинг всех букмекеров параллельно"""
        results = await asyncio.gather(
            self.parse_winline(),
            self.parse_olimp(),
            self.parse_pari(),
            self.parse_fonbet(),
            return_exceptions=True
        )
        
        all_events = []
        for result in results:
            if isinstance(result, list):
                all_events.extend(result)
        
        return all_events
