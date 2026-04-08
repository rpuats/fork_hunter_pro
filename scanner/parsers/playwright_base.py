# scanner/parsers/playwright_base.py
"""
Playwright-based base parser for Russian bookmakers
Intercepts network requests to extract real odds data
"""
import asyncio
import json
import re
import time
from typing import List, Dict, Optional, Any
from abc import ABC, abstractmethod
import logging
from playwright.async_api import async_playwright, Browser, BrowserContext, Page, Route, Request

logger = logging.getLogger(__name__)


class PlaywrightParser(ABC):
    """Base class for Playwright-based bookmaker parsers"""
    
    name: str = "base"
    slug: str = "base"
    base_url: str = ""
    live_path: str = "/live"
    prematch_path: str = "/line"
    
    def __init__(self):
        self.browser: Optional[Browser] = None
        self.context: Optional[BrowserContext] = None
        self.page: Optional[Page] = None
        self._captured_data: List[Dict] = []
        self._request_count = 0
        self._errors = 0
        self._last_update = 0
    
    async def init(self):
        """Initialize browser"""
        pw = await async_playwright().start()
        self.browser = await pw.chromium.launch(
            headless=True,
            args=[
                '--disable-blink-features=AutomationControlled',
                '--no-sandbox',
                '--disable-dev-shm-usage',
                '--disable-web-security',
            ]
        )
        self.context = await self.browser.new_context(
            user_agent='Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            viewport={'width': 1920, 'height': 1080},
            locale='ru-RU',
            timezone_id='Europe/Moscow',
        )
        # Anti-detection
        await self.context.add_init_script("""
            Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
            window.chrome = { runtime: {} };
            Object.defineProperty(navigator, 'languages', { get: () => ['ru-RU', 'ru', 'en-US', 'en'] });
            Object.defineProperty(navigator, 'plugins', { get: () => [1, 2, 3, 4, 5] });
        """)
    
    async def close(self):
        """Close browser"""
        if self.context:
            await self.context.close()
        if self.browser:
            await self.browser.close()
    
    async def _setup_interceptors(self):
        """Setup network request interceptors"""
        assert self.page is not None
        
        @self.page.route("**/api/**", self._intercept_api)
        @self.page.route("**/feed/**", self._intercept_feed)
        @self.page.route("**/line/**", self._intercept_line)
        @self.page.route("**/live/**", self._intercept_live)
        @self.page.route("**/odds/**", self._intercept_odds)
        @self.page.route("**/events/**", self._intercept_events)
        async def catch_all(route: Route, request: Request):
            url = request.url
            if any(p in url.lower() for p in ['api', 'feed', 'line', 'live', 'odds', 'events', 'json']):
                try:
                    response = await route.fetch()
                    body = await response.text()
                    if body and len(body) > 100:
                        self._process_response(url, body)
                    await route.fulfill(response=response)
                except:
                    await route.continue_()
            else:
                await route.continue_()
    
    async def _intercept_api(self, route: Route, request: Request):
        await self._handle_route(route, request)
    
    async def _intercept_feed(self, route: Route, request: Request):
        await self._handle_route(route, request)
    
    async def _intercept_line(self, route: Route, request: Request):
        await self._handle_route(route, request)
    
    async def _intercept_live(self, route: Route, request: Request):
        await self._handle_route(route, request)
    
    async def _intercept_odds(self, route: Route, request: Request):
        await self._handle_route(route, request)
    
    async def _intercept_events(self, route: Route, request: Request):
        await self._handle_route(route, request)
    
    async def _handle_route(self, route: Route, request: Request):
        """Handle intercepted route"""
        try:
            response = await route.fetch()
            body = await response.text()
            if body and len(body) > 100:
                self._process_response(request.url, body)
            await route.fulfill(response=response)
        except:
            await route.continue_()
    
    def _process_response(self, url: str, body: str):
        """Process intercepted response"""
        try:
            data = json.loads(body)
            events = self._extract_events_from_json(data, url)
            if events:
                self._captured_data.extend(events)
                logger.debug(f"[{self.slug}] Captured {len(events)} events from {url}")
        except json.JSONDecodeError:
            # Try to find JSON in HTML/JS
            json_matches = re.findall(r'(\{[^{}]*"odds"[^{}]*\})', body)
            for match in json_matches:
                try:
                    data = json.loads(match)
                    events = self._extract_events_from_json(data, url)
                    if events:
                        self._captured_data.extend(events)
                except:
                    pass
    
    def _extract_events_from_json(self, data: Any, url: str) -> List[Dict]:
        """Extract events from JSON data - override in subclasses"""
        return []
    
    async def get_events(self) -> List[Dict]:
        """Get events using Playwright"""
        self._captured_data = []
        
        if not self.browser:
            await self.init()
        
        try:
            assert self.context is not None
            self.page = await self.context.new_page()
            
            await self._setup_interceptors()
            
            # Navigate to live page
            live_url = f"{self.base_url}{self.live_path}"
            logger.info(f"[{self.slug}] Navigating to {live_url}")
            
            await self.page.goto(live_url, wait_until="domcontentloaded", timeout=30000)
            
            # Wait for dynamic content
            await asyncio.sleep(5)
            
            # Scroll to trigger lazy loading
            await self.page.evaluate("""
                () => {
                    window.scrollBy(0, 500);
                    setTimeout(() => window.scrollBy(0, 500), 500);
                    setTimeout(() => window.scrollBy(0, 500), 1000);
                }
            """)
            await asyncio.sleep(3)
            
            # Also try prematch
            prematch_url = f"{self.base_url}{self.prematch_path}"
            try:
                await self.page.goto(prematch_url, wait_until="domcontentloaded", timeout=20000)
                await asyncio.sleep(3)
            except:
                pass
            
            events = self._captured_data[:100]
            self._last_update = time.time()
            self._request_count += 1
            
            logger.info(f"[{self.slug}] Captured {len(events)} events")
            return events
            
        except Exception as e:
            self._errors += 1
            logger.error(f"[{self.slug}] Error: {e}")
            return []
        finally:
            if self.page:
                await self.page.close()
                self.page = None
    
    def get_stats(self) -> Dict:
        return {
            'name': self.name,
            'slug': self.slug,
            'requests': self._request_count,
            'errors': self._errors,
            'last_update': self._last_update,
            'error_rate': round(self._errors / max(self._request_count, 1) * 100, 2)
        }
