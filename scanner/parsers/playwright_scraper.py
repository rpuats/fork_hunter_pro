# scanner/parsers/playwright_scraper.py
import asyncio
from typing import List, Dict, Optional
from datetime import datetime
import re


class PlaywrightScraper:
    """Playwright-based scraper для сложных сайтов"""
    
    def __init__(self):
        self.browser = None
        self.context = None
        self.playwright = None
    
    async def init(self):
        try:
            from playwright.async_api import async_playwright
            self.playwright = await async_playwright().start()
            self.browser = await self.playwright.chromium.launch(headless=True)
            self.context = await self.browser.new_context(
                user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
            )
        except ImportError:
            print("Playwright not installed. Run: pip install playwright && playwright install")
        except Exception as e:
            print(f"Playwright init error: {e}")
    
    async def close(self):
        if self.browser:
            await self.browser.close()
        if self.playwright:
            await self.playwright.stop()
    
    async def scrape_winline(self) -> List[Dict]:
        if not self.browser:
            return []
        
        events = []
        page = None
        
        try:
            page = await self.context.new_page()
            await page.goto("https://winline.ru/live", wait_until="networkidle", timeout=30000)
            await page.wait_for_timeout(2000)
            
            content = await page.content()
            events.extend(self._parse_winline_page(content))
            
        except Exception as e:
            print(f"Winline scrape error: {e}")
        finally:
            if page:
                await page.close()
        
        return events[:30]
    
    def _parse_winline_page(self, html: str) -> List[Dict]:
        events = []
        
        patterns = [
            r'"team1":"([^"]+)".*?"team2":"([^"]+)".*?"k1":([0-9.]+).*?"k2":([0-9.]+)',
            r'data-match="([^"]+)".*?data-k1="([0-9.]+)".*?data-k2="([0-9.]+)"',
        ]
        
        for pattern in patterns:
            matches = re.findall(pattern, html)
            for match in matches:
                try:
                    if len(match) >= 4:
                        team1, team2, k1, k2 = match[0], match[1], float(match[2]), float(match[3])
                    else:
                        continue
                    
                    if k1 > 1.01 or k2 > 1.01:
                        events.append({
                            'id': str(hash(team1 + team2)),
                            'bookmaker': 'winline',
                            'sport': 'football',
                            'home_team': team1,
                            'away_team': team2,
                            'league': 'Live',
                            'home_odds': k1,
                            'away_odds': k2,
                            'is_live': True,
                            'market': '1x2'
                        })
                except:
                    continue
        
        return events
    
    async def scrape_olimp(self) -> List[Dict]:
        if not self.browser:
            return []
        
        events = []
        page = None
        
        try:
            page = await self.context.new_page()
            await page.goto("https://www.olimp.bet/live", wait_until="networkidle", timeout=30000)
            await page.wait_for_timeout(2000)
            
            content = await page.content()
            events.extend(self._parse_olimp_page(content))
            
        except Exception as e:
            print(f"Olimp scrape error: {e}")
        finally:
            if page:
                await page.close()
        
        return events[:30]
    
    def _parse_olimp_page(self, html: str) -> List[Dict]:
        events = []
        
        pattern = r'"O1":"([^"]+)".*?"O2":"([^"]+)".*?"C1":([0-9.]+).*?"C2":([0-9.]+)'
        matches = re.findall(pattern, html)
        
        for match in matches:
            try:
                o1, o2, c1, c2 = match[0], match[1], float(match[2]), float(match[3])
                
                if c1 > 1.01 or c2 > 1.01:
                    events.append({
                        'id': str(hash(o1 + o2)),
                        'bookmaker': 'olimp',
                        'sport': 'football',
                        'home_team': o1,
                        'away_team': o2,
                        'league': 'Live',
                        'home_odds': c1,
                        'away_odds': c2,
                        'is_live': True,
                        'market': '1x2'
                    })
            except:
                continue
        
        return events
    
    async def scrape_pari(self) -> List[Dict]:
        if not self.browser:
            return []
        
        events = []
        page = None
        
        try:
            page = await self.context.new_page()
            await page.goto("https://www.pari.ru/live", wait_until="networkidle", timeout=30000)
            await page.wait_for_timeout(3000)
            
            content = await page.content()
            events.extend(self._parse_pari_page(content))
            
        except Exception as e:
            print(f"Pari scrape error: {e}")
        finally:
            if page:
                await page.close()
        
        return events[:30]
    
    def _parse_pari_page(self, html: str) -> List[Dict]:
        events = []
        
        pattern = r'"team1":"([^"]+)".*?"team2":"([^"]+)".*?"win1":([0-9.]+).*?"win2":([0-9.]+)'
        matches = re.findall(pattern, html)
        
        for match in matches:
            try:
                t1, t2, w1, w2 = match[0], match[1], float(match[2]), float(match[3])
                
                if w1 > 1.01 or w2 > 1.01:
                    events.append({
                        'id': str(hash(t1 + t2)),
                        'bookmaker': 'pari',
                        'sport': 'football',
                        'home_team': t1,
                        'away_team': t2,
                        'league': 'Live',
                        'home_odds': w1,
                        'away_odds': w2,
                        'is_live': True,
                        'market': '1x2'
                    })
            except:
                continue
        
        return events
    
    async def scrape_betboom(self) -> List[Dict]:
        if not self.browser:
            return []
        
        events = []
        page = None
        
        try:
            page = await self.context.new_page()
            await page.goto("https://betboom.ru/live", wait_until="networkidle", timeout=30000)
            await page.wait_for_timeout(3000)
            
            content = await page.content()
            events.extend(self._parse_betboom_page(content))
            
        except Exception as e:
            print(f"BetBoom scrape error: {e}")
        finally:
            if page:
                await page.close()
        
        return events[:30]
    
    def _parse_betboom_page(self, html: str) -> List[Dict]:
        events = []
        
        pattern = r'"team1":"([^"]+)".*?"team2":"([^"]+)".*?"coefficient1":([0-9.]+).*?"coefficient2":([0-9.]+)'
        matches = re.findall(pattern, html)
        
        for match in matches:
            try:
                t1, t2, c1, c2 = match[0], match[1], float(match[2]), float(match[3])
                
                if c1 > 1.01 or c2 > 1.01:
                    events.append({
                        'id': str(hash(t1 + t2)),
                        'bookmaker': 'betboom',
                        'sport': 'football',
                        'home_team': t1,
                        'away_team': t2,
                        'league': 'Live',
                        'home_odds': c1,
                        'away_odds': c2,
                        'is_live': True,
                        'market': '1x2'
                    })
            except:
                continue
        
        return events
    
    async def scrape_all(self) -> List[Dict]:
        """Парсинг всех сайтов через Playwright"""
        all_events = []
        
        results = await asyncio.gather(
            self.scrape_winline(),
            self.scrape_olimp(),
            self.scrape_pari(),
            self.scrape_betboom(),
            return_exceptions=True
        )
        
        for result in results:
            if isinstance(result, list):
                all_events.extend(result)
        
        return all_events
