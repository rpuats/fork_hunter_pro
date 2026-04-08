# scanner/parsers/ligastavok_playwright.py
"""
Liga Stavok Playwright Parser
Uses Playwright to bypass QRATOR, then calls API from page context via fetch().
API endpoints discovered via network capture.
"""
import asyncio
import json
import time
import uuid
import logging
from typing import List, Dict
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class LigaStavokPlaywrightParser:
    """Liga Stavok parser using Playwright + API from page context."""
    
    name = "Liga Stavok (Playwright)"
    slug = "ligastavok"
    base_url = "https://www.ligastavok.ru"
    api_host = "https://lds-api-sites.ligastavok.ru"
    events_list_url = f"{api_host}/rest/events/v8/eventsList"
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        try:
            pw = await async_playwright().start()
            browser = await pw.chromium.launch(
                headless=False,  # Must be non-headless for QRATOR
                args=['--disable-blink-features=AutomationControlled', '--no-sandbox']
            )
            context = await browser.new_context(
                user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
                viewport={'width': 1920, 'height': 1080},
                locale='ru-RU',
                timezone_id='Europe/Moscow',
            )
            page = await context.new_page()
            
            # Step 1: Navigate to site and pass QRATOR
            logger.info("[ligastavok] Passing QRATOR...")
            await page.goto(self.base_url + '/line/football', wait_until='domcontentloaded', timeout=30000)
            await asyncio.sleep(15)  # Wait for QRATOR JS challenge
            
            # Simulate human interaction
            await page.mouse.move(500, 500)
            await page.mouse.click(500, 500)
            await asyncio.sleep(5)
            
            # Check if page loaded
            title = await page.title()
            logger.info(f"[ligastavok] Page title: {title}")
            
            # Step 2: Call API from WITHIN page context (uses browser's cookies + TLS fingerprint)
            logger.info("[ligastavok] Calling API from page context...")
            payload = {
                "gameId": [],
                "limit": 200,
                "skip": 0,
                "topEvents": False,
                "ts": int(time.time() * 1000),
                "view": "priority",
                "widgetVideo": False,
                "proposedTypes": ["MAINOFFER"]
            }
            
            # Use page.evaluate to make fetch() from within the page context
            api_result = await page.evaluate(f"""
                async () => {{
                    try {{
                        const response = await fetch('{self.events_list_url}', {{
                            method: 'POST',
                            headers: {{
                                'Content-Type': 'application/json',
                                'x-application-name': 'mobile',
                                'x-req-id': '{uuid.uuid4()}'
                            }},
                            body: JSON.stringify({json.dumps(payload)})
                        }});
                        if (response.ok) {{
                            return await response.json();
                        }}
                        return null;
                    }} catch (e) {{
                        console.error('API call failed:', e);
                        return null;
                    }}
                }}
            """)
            
            if api_result:
                all_events = self._parse_response(api_result)
                logger.info(f"[ligastavok] Got {len(all_events)} events from API")
            else:
                logger.warning("[ligastavok] API returned no data, trying interception fallback...")
                # Fallback: intercept API response from page reload
                api_data = []
                async def handle_response(response):
                    if self.events_list_url in response.url and response.status == 200:
                        try:
                            data = await response.json()
                            api_data.append(data)
                        except:
                            pass
                
                page.on('response', handle_response)
                await page.reload(wait_until='networkidle', timeout=30000)
                await asyncio.sleep(8)
                
                if api_data:
                    all_events = self._parse_response(api_data[0])
                    logger.info(f"[ligastavok] Got {len(all_events)} events via interception")
            
            await browser.close()
            
        except Exception as e:
            logger.warning(f"[ligastavok] Error: {e}")
        
        return all_events
    
    def _parse_response(self, data) -> List[Dict]:
        """Parse API response into events"""
        events = []
        if not isinstance(data, dict):
            return events
        
        result = data.get('result', {})
        if not isinstance(result, dict):
            return events
        
        items = result.get('data', [])
        
        for item in items:
            try:
                event_info = item.get('event', {})
                team1 = event_info.get('team1', '').strip()
                team2 = event_info.get('team2', '').strip()
                
                if not team1 or not team2:
                    continue
                
                outcomes = item.get('outcomes', {})
                if not isinstance(outcomes, dict):
                    continue
                
                home_odds = draw_odds = away_odds = 0.0
                totals = []
                handicaps = []
                
                for key, out in outcomes.items():
                    if not isinstance(out, dict):
                        continue
                    title = out.get('title', '')
                    value = float(out.get('value', 0))
                    ad_value = out.get('adValue')
                    
                    if title == '1':
                        home_odds = value
                    elif title == 'X':
                        draw_odds = value
                    elif title == '2':
                        away_odds = value
                    elif title in ['Мен', 'Меньше', 'Under', 'ТМ']:
                        totals.append({
                            'type': 'under',
                            'value': float(ad_value) if ad_value else None,
                            'odd': value
                        })
                    elif title in ['Бол', 'Больше', 'Over', 'ТБ']:
                        totals.append({
                            'type': 'over',
                            'value': float(ad_value) if ad_value else None,
                            'odd': value
                        })
                    elif title.startswith('Ф') or title.startswith('H'):
                        handicaps.append({
                            'type': 'handicap',
                            'value': float(ad_value) if ad_value else None,
                            'odd': value,
                            'team': '1' if '1' in title else '2'
                        })
                
                if home_odds > 1 and draw_odds > 1 and away_odds > 1:
                    is_live = item.get('ns') == 'live'
                    events.append({
                        'id': f"ligastavok_{item.get('id')}",
                        'bookmaker': 'ligastavok',
                        'sport': event_info.get('gameTitle', 'football'),
                        'home_team': team1,
                        'away_team': team2,
                        'league': event_info.get('tournamentTitle', 'Unknown'),
                        'home_odds': home_odds,
                        'draw_odds': draw_odds,
                        'away_odds': away_odds,
                        'totals': totals,
                        'handicaps': handicaps,
                        'is_live': is_live,
                        'market': '1x2',
                        'source_url': f"{self.base_url}/live" if is_live else f"{self.base_url}/line",
                        'scraped_at': time.time()
                    })
            except Exception as e:
                logger.debug(f"[ligastavok] Parse error: {e}")
        
        return events
