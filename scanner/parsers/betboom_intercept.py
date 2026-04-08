# scanner/parsers/betboom_intercept.py
"""
Betboom Parser - API Interception
Intercepts API responses instead of parsing DOM
"""
import asyncio
import json
import time
from datetime import datetime
from typing import List, Dict, Optional
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class BetboomInterceptParser:
    """Betboom parser using API interception."""
    
    name = "Betboom (Intercept)"
    slug = "betboom"
    
    def __init__(self):
        self.api_data = []
        self.events = []
    
    async def _intercept_response(self, response):
        """Intercept API responses and save JSON."""
        url = response.url
        if response.status == 200:
            ct = response.headers.get('content-type', '')
            if 'json' in ct and len(url) > 20:
                try:
                    data = await response.json()
                    if isinstance(data, (dict, list)) and len(str(data)) > 100:
                        self.api_data.append({'url': url, 'data': data})
                        logger.debug(f"Intercepted: {url[:60]} ({len(str(data))} bytes)")
                except:
                    pass
    
    def _extract_events_from_data(self, data: Dict, url: str) -> List[Dict]:
        """Extract events from API response data."""
        events = []
        
        # Try different data structures
        matches = []
        
        # Structure 1: data.matches
        if 'matches' in data:
            matches = data['matches']
        # Structure 2: data.events
        elif 'events' in data:
            matches = data['events']
        # Structure 3: data.data.matches
        elif 'data' in data and isinstance(data['data'], dict):
            inner = data['data']
            if 'matches' in inner:
                matches = inner['matches']
            elif 'events' in inner:
                matches = inner['events']
        # Structure 4: list of items
        elif isinstance(data, list):
            matches = data
        
        for match in matches:
            if not isinstance(match, dict):
                continue
            
            # Try to get team names
            home = ''
            away = ''
            
            # Pattern 1: homeTeam/awayTeam
            if 'homeTeam' in match:
                ht = match['homeTeam']
                home = ht.get('name', ht) if isinstance(ht, dict) else str(ht)
            elif 'home' in match:
                home = str(match['home'])
            elif 'team1' in match:
                home = str(match['team1'])
            
            if 'awayTeam' in match:
                at = match['awayTeam']
                away = at.get('name', at) if isinstance(at, dict) else str(at)
            elif 'away' in match:
                away = str(match['away'])
            elif 'team2' in match:
                away = str(match['team2'])
            
            # Try to get odds
            home_odds = 0
            draw_odds = 0
            away_odds = 0
            
            # Pattern 1: odds.homeWin
            if 'odds' in match:
                odds = match['odds']
                if isinstance(odds, dict):
                    home_odds = float(odds.get('homeWin', odds.get('home', 0)) or 0)
                    draw_odds = float(odds.get('draw', odds.get('X', 0)) or 0)
                    away_odds = float(odds.get('awayWin', odds.get('away', 0)) or 0)
            
            # Pattern 2: markets
            if home_odds == 0 and 'markets' in match:
                for market in match['markets']:
                    if not isinstance(market, dict):
                        continue
                    outcomes = market.get('outcomes', [])
                    for outcome in outcomes:
                        if not isinstance(outcome, dict):
                            continue
                        otype = outcome.get('type', '')
                        oprice = float(outcome.get('odds', outcome.get('price', 0)) or 0)
                        if 'home' in otype.lower() or otype == '1':
                            home_odds = oprice
                        elif 'draw' in otype.lower() or otype == 'X':
                            draw_odds = oprice
                        elif 'away' in otype.lower() or otype == '2':
                            away_odds = oprice
            
            if home and away and (home_odds > 1 or away_odds > 1):
                events.append({
                    'id': f"betboom_{hash(str(home) + str(away))}",
                    'bookmaker': 'betboom',
                    'sport': 'football',
                    'home_team': str(home),
                    'away_team': str(away),
                    'league': 'Live' if 'live' in url.lower() else 'Pre-match',
                    'home_odds': home_odds,
                    'draw_odds': draw_odds if draw_odds > 1 else None,
                    'away_odds': away_odds,
                    'is_live': 'live' in url.lower(),
                    'market': '1x2',
                    'source_url': url,
                    'scraped_at': time.time()
                })
        
        return events
    
    async def get_events(self) -> List[Dict]:
        """Main method: load page, intercept API, extract events."""
        self.api_data = []
        
        urls = [
            "https://betboom.ru/sport/football",
            "https://betboom.ru/sport/live",
            "https://betboom.ru/line/football",
        ]
        
        all_events = []
        
        for url in urls:
            try:
                pw = await async_playwright().start()
                browser = await pw.chromium.launch(
                    headless=True,
                    args=['--disable-blink-features=AutomationControlled']
                )
                context = await browser.new_context(
                    user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
                    viewport={'width': 1920, 'height': 1080},
                    locale='ru-RU',
                    timezone_id='Europe/Moscow',
                )
                page = await context.new_page()
                
                await page.add_init_script("""
                    Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
                    window.chrome = {runtime: {}};
                """)
                
                page.on('response', lambda r: asyncio.create_task(self._intercept_response(r)))
                
                await page.goto(url, wait_until='domcontentloaded', timeout=30000)
                await asyncio.sleep(10)
                
                # Extract events from intercepted data
                for item in self.api_data:
                    events = self._extract_events_from_data(item['data'], item['url'])
                    all_events.extend(events)
                
                logger.info(f"Betboom ({url}): {len(all_events)} events from {len(self.api_data)} API calls")
                
                await browser.close()
                
                if all_events:
                    break
                    
            except Exception as e:
                logger.warning(f"Betboom failed for {url}: {e}")
        
        return all_events
