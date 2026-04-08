# scanner/parsers/ligastavok_api.py
"""
Liga Stavok Parser - Uses Playwright to intercept API responses
API: https://lds-api-sites.ligastavok.ru/rest/events/v8/eventsList
"""
import asyncio
import time
import logging
from typing import List, Dict
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)

class LigaStavokPlaywrightParser:
    name = "Liga Stavok (Intercept)"
    slug = "ligastavok"
    
    API_URL = "https://lds-api-sites.ligastavok.ru/rest/events/v8/eventsList"
    BASE_URL = "https://www.ligastavok.ru"
    
    async def get_events(self) -> List[Dict]:
        """Fetch events by intercepting API responses via Playwright"""
        all_events = []
        api_responses = []
        
        try:
            async with async_playwright() as p:
                browser = await p.chromium.launch(headless=True)
                context = await browser.new_context(
                    user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36',
                    viewport={'width': 1920, 'height': 1080},
                    locale='ru-RU',
                )
                
                # Intercept API responses
                async def handle_response(response):
                    if self.API_URL in response.url and response.status == 200:
                        try:
                            data = await response.json()
                            api_responses.append(data)
                        except:
                            pass
                
                page = await context.new_page()
                page.on('response', handle_response)
                
                # Visit pages to trigger API calls
                urls_to_visit = [
                    self.BASE_URL + '/live/football',
                    self.BASE_URL + '/line/football',
                ]
                
                for url in urls_to_visit:
                    try:
                        await page.goto(url, wait_until='networkidle', timeout=30000)
                        await asyncio.sleep(3)
                        # Scroll to load more
                        for i in range(5):
                            await page.evaluate("window.scrollTo(0, document.body.scrollHeight / 5 * {})".format(i+1))
                            await asyncio.sleep(0.5)
                    except Exception as e:
                        logger.warning("LigaStavok page error: {}".format(e))
                
                # Parse all intercepted responses
                for data in api_responses:
                    batch_events = self._parse_response(data)
                    all_events.extend(batch_events)
                
                logger.info("LigaStavok: {} events from {} API responses".format(len(all_events), len(api_responses)))
                
        except Exception as e:
            logger.warning("LigaStavok error: {}".format(e))
        
        return all_events
    
    def _parse_response(self, data) -> List[Dict]:
        """Parse API response"""
        events = []
        if not isinstance(data, dict):
            return events
        
        result = data.get('result', {})
        if not isinstance(result, dict):
            return events
        
        items = result.get('data', [])
        
        for item in items:
            try:
                event_data = item.get('event', {})
                team1 = event_data.get('team1', '')
                team2 = event_data.get('team2', '')
                
                if not team1 or not team2:
                    continue
                
                outcomes = item.get('outcomes', {})
                if not isinstance(outcomes, dict):
                    continue
                
                home_odds = 0.0
                draw_odds = 0.0
                away_odds = 0.0
                
                for outcome in outcomes.values():
                    if not isinstance(outcome, dict):
                        continue
                    title = outcome.get('title', '')
                    value = float(outcome.get('value', 0))
                    
                    if title == '1':
                        home_odds = value
                    elif title == 'X':
                        draw_odds = value
                    elif title == '2':
                        away_odds = value
                
                if home_odds > 1 and draw_odds > 1 and away_odds > 1:
                    is_live = item.get('ns') == 'live'
                    events.append({
                        'id': "ligastavok_{}".format(item.get('id', hash(team1 + team2))),
                        'bookmaker': 'ligastavok',
                        'sport': 'football',
                        'home_team': team1,
                        'away_team': team2,
                        'league': 'Live' if is_live else 'Pre-match',
                        'home_odds': home_odds,
                        'draw_odds': draw_odds,
                        'away_odds': away_odds,
                        'is_live': is_live,
                        'market': '1x2',
                        'source_url': 'https://www.ligastavok.ru/live',
                        'scraped_at': time.time()
                    })
            except Exception as e:
                logger.debug("LigaStavok item parse error: {}".format(e))
        
        return events
