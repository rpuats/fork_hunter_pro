# scanner/parsers/betboom_playwright.py
"""
BetBoom Parser - Text extraction from main page
"""
import asyncio
import time
from typing import List, Dict
import logging
from playwright.async_api import async_playwright

logger = logging.getLogger(__name__)


class BetBoomPlaywrightParser:
    name = "BetBoom (Playwright)"
    slug = "betboom"
    urls = [
        "https://betboom.ru/sport/football",
        "https://betboom.ru/sport/live/football",
        "https://betboom.ru/sport/basketball",
        "https://betboom.ru/sport/live/basketball",
        "https://betboom.ru/sport/hockey",
        "https://betboom.ru/sport/live/hockey",
        "https://betboom.ru/sport/tennis",
        "https://betboom.ru/sport/live/tennis",
        "https://betboom.ru/sport/volleyball",
        "https://betboom.ru/sport/live/volleyball",
        "https://betboom.ru/sport/baseball",
        "https://betboom.ru/sport/live/baseball",
        "https://betboom.ru/sport/handball",
        "https://betboom.ru/sport/live/handball",
        "https://betboom.ru/sport/rugby",
        "https://betboom.ru/sport/live/rugby",
        "https://betboom.ru/sport/table-tennis",
        "https://betboom.ru/sport/live/table-tennis",
        "https://betboom.ru/sport/badminton",
        "https://betboom.ru/sport/live/badminton",
    ]
    
    async def get_events(self) -> List[Dict]:
        all_events = []
        for url in self.urls:
            try:
                events = await self._fetch_url(url)
                all_events.extend(events)
                if events:
                    break
            except Exception as e:
                logger.warning(f"BetBoom failed for {url}: {e}")
        return all_events
    
    async def _fetch_url(self, url: str) -> List[Dict]:
        pw = await async_playwright().start()
        browser = await pw.chromium.launch(
            headless=True,
            args=['--disable-blink-features=AutomationControlled']
        )
        context = await browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
        )
        await context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            window.chrome = {runtime: {}};
        """)
        
        page = await context.new_page()
        
        # Intercept API calls
        api_data = []
        async def handle_response(response):
            url = response.url
            ct = response.headers.get('content-type', '')
            if response.status == 200 and 'json' in ct:
                try:
                    data = await response.json()
                    if isinstance(data, dict) and len(str(data)) > 100:
                        api_data.append({'url': url, 'data': data})
                except:
                    pass
        
        page.on('response', handle_response)
        
        try:
            await page.goto(url, wait_until='domcontentloaded', timeout=30000)
            await asyncio.sleep(15)
            
            # Try to extract from iframes
            events = []
            for frame in page.frames:
                if frame != page.main_frame and 'betboom' in frame.url:
                    try:
                        frame_events = await self._extract_from_frame(frame, url)
                        events.extend(frame_events)
                    except:
                        pass
            
            # If no events from iframes, try text extraction from main page
            if not events:
                events = await self._extract_from_text(page, url)
            
            logger.info(f"BetBoom ({url}): {len(events)} events")
        except Exception as e:
            logger.warning(f"BetBoom error: {e}")
            events = []
        finally:
            await browser.close()
        
        return events
    
    async def _extract_from_frame(self, frame, url: str) -> List[Dict]:
        """Extract events from iframe."""
        try:
            raw_events = await frame.evaluate("""
                () => {
                    const events = [];
                    const containers = document.querySelectorAll('[class*="event"], [class*="match"], [class*="coupon"], [class*="card"]');
                    
                    containers.forEach(el => {
                        const text = el.textContent || '';
                        if (!text || text.length < 20) return;
                        
                        const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                        const teams = [];
                        const odds = [];
                        
                        for (const line of lines) {
                            const val = parseFloat(line.replace(',', '.'));
                            if (!isNaN(val) && val >= 1.01 && val <= 50) {
                                odds.push(val);
                            } else if (line.length > 2 && line.length < 40) {
                                teams.push(line);
                            }
                            if (teams.length >= 2 && odds.length >= 1) break;
                        }
                        
                        if (teams.length >= 2 && odds.length >= 1) {
                            events.push({
                                home: teams[0],
                                away: teams[1],
                                odds: odds.slice(0, 4)
                            });
                        }
                    });
                    
                    return events;
                }
            """)
            
            return self._normalize(raw_events, url)
        except:
            return []
    
    async def _extract_from_text(self, page, url: str) -> List[Dict]:
        """Extract events from page text."""
        try:
            raw_events = await page.evaluate("""
                () => {
                    const events = [];
                    const text = document.body.innerText;
                    const lines = text.split('\\n').map(l => l.trim()).filter(l => l.length > 1);
                    
                    let i = 0;
                    while (i < lines.length - 2) {
                        const line = lines[i];
                        
                        if (line.length > 3 && line.length < 50 && 
                            !line.match(/^\\d/) && !line.match(/LIVE|live/i) &&
                            !line.match(/^\\d+[.,]\\d+$/)) {
                            
                            const odds = [];
                            const teamLines = [line];
                            let j = i + 1;
                            
                            while (j < lines.length && odds.length < 6) {
                                const val = parseFloat(lines[j].replace(',', '.'));
                                if (!isNaN(val) && val >= 1.01 && val <= 50) {
                                    odds.push(val);
                                } else if (lines[j].length > 3 && lines[j].length < 50 && teamLines.length < 2) {
                                    teamLines.push(lines[j]);
                                }
                                j++;
                                if (odds.length >= 2 && teamLines.length >= 2) break;
                            }
                            
                            if (teamLines.length >= 2 && odds.length >= 1) {
                                events.push({
                                    home: teamLines[0],
                                    away: teamLines[teamLines.length - 1],
                                    odds: odds.slice(0, 4)
                                });
                                i = j;
                                continue;
                            }
                        }
                        i++;
                    }
                    
                    return events;
                }
            """)
            
            return self._normalize(raw_events, url)
        except:
            return []
    
    def _normalize(self, raw_events: list, url: str) -> List[Dict]:
        result = []
        seen = set()
        
        for e in raw_events:
            home = e.get('home', '').strip()
            away = e.get('away', '').strip()
            odds = e.get('odds', [])
            
            if not home or not away or len(home) < 2 or len(away) < 2:
                continue
            
            key = f"{home}|{away}"
            if key in seen:
                continue
            seen.add(key)
            
            if len(odds) < 1:
                continue
            
            result.append({
                'id': f"betboom_{hash(key) % 1000000}",
                'bookmaker': 'betboom',
                'sport': 'football',
                'home_team': home,
                'away_team': away,
                'league': 'Live' if 'live' in url else 'Pre-match',
                'home_odds': odds[0],
                'draw_odds': odds[1] if len(odds) > 2 else None,
                'away_odds': odds[2] if len(odds) > 2 else (odds[1] if len(odds) > 1 else 0),
                'is_live': 'live' in url,
                'market': '1x2',
                'source_url': url,
                'scraped_at': time.time()
            })
        
        return result
