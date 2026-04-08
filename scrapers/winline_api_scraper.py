# scrapers/winline_api_scraper.py
import asyncio
import json
import re
import logging
from typing import List, Dict

import aiohttp
from playwright.async_api import async_playwright

from scrapers.base_scraper import BaseScraper
from core.event_normalizer import normalize_event_name

logger = logging.getLogger(__name__)

class WinlineAPIScraper(BaseScraper):
    def __init__(self):
        super().__init__()
        self.name = "Winline"
        self.session = None
        self.cookies = None

    async def init_session(self):
        """Получаем cookies через Playwright один раз"""
        if self.cookies:
            return

        async with async_playwright() as p:
            browser = await p.chromium.launch(headless=True)
            page = await browser.new_page()
            await page.goto("https://winline.ru/live", wait_until="domcontentloaded", timeout=60000)
            await asyncio.sleep(8)
            
            self.cookies = await page.context.cookies()
            await browser.close()

        self.session = aiohttp.ClientSession(cookies={c['name']: c['value'] for c in self.cookies})

    async def get_events(self) -> List[Dict]:
        await self.init_session()
        events = []

        try:
            # Основной эндпоинт live (нужно найти актуальный)
            async with self.session.get("https://winline.ru/api/live/events", timeout=10) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    events.extend(self.parse_api_response(data))

            # Альтернативный эндпоинт
            async with self.session.get("https://winline.ru/api/v2/live", timeout=10) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    events.extend(self.parse_api_response(data))

        except Exception as e:
            logger.error(f"[Winline API] Error: {e}")

        logger.info(f"[Winline] Получено {len(events)} событий через API")
        return events

    def parse_api_response(self, data):
        events = []
        # Рекурсивный поиск событий в JSON
        def extract(item):
            if isinstance(item, dict):
                if "name" in item and "odds" in item:
                    name = item.get("name")
                    odds = item.get("odds", {})
                    if isinstance(name, str) and len(name) > 15:
                        normalized = normalize_event_name(name)
                        events.append({
                            'name': name,
                            'normalized_name': normalized,
                            'market_type': '1x2',
                            'p1': odds.get("1", 0) or odds.get("p1", 0),
                            'p2': odds.get("2", 0) or odds.get("p2", 0),
                            'bookmaker': 'Winline'
                        })
                for v in item.values():
                    extract(v)
            elif isinstance(item, list):
                for i in item:
                    extract(i)

        extract(data)
        return events
