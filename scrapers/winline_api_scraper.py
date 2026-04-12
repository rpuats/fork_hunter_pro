# scrapers/winline_api_scraper.py - HTTP-based scraper for prematch
import asyncio
import re
import logging
from typing import List, Dict
import requests
from bs4 import BeautifulSoup

from scrapers.base_scraper import BaseScraper
from core.event_normalizer import normalize_event_name

logger = logging.getLogger(__name__)

class WinlineAPIScraper(BaseScraper):
    def __init__(self):
        super().__init__()
        self.name = "Winline"

        # Filter for Winline
        self.exclude_patterns = [
            r'избранное', r'ближайшие', r'корзина', r'история', r'бонус',
            r'акция', r'кешбэк', r'генератор экспресса', r'размер коэффициента',
            r'сумма возм.выигрыша', r'только топ-события', r'добавить исход',
            r'обновить список', r'популярные события', r'добавить в корзину',
            r'подробнее', r'ежемесячно', r'деньгами', r'личный кабинет',
            r'пополнение', r'вывод', r'правила', r'помощь', r'поддержка'
        ]

    async def get_events(self) -> List[Dict]:
        events = []
        try:
            url = "https://winline.ru/line"
            events = await self._scrape_url(url)
        except Exception as e:
            logger.error(f"[Winline] Error: {e}")

        logger.info(f"[Winline] Collected {len(events)} events")
        return events

    async def _scrape_url(self, url: str) -> List[Dict]:
        """Scrape events from URL"""
        events = []

        loop = asyncio.get_event_loop()
        html_content = await loop.run_in_executor(None, self._fetch_page, url)

        if not html_content:
            return events

        soup = BeautifulSoup(html_content, 'html.parser')
        text_blocks = self._extract_text_blocks(soup)

        for block in text_blocks:
            try:
                event = self._parse_event_block(block)
                if event:
                    events.append(event)
            except Exception as e:
                logger.debug(f"[Winline] Failed to parse block: {e}")
                continue

        return events

    def _fetch_page(self, url: str) -> str:
        """Fetch page content"""
        headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            'Accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
            'Accept-Language': 'ru-RU,ru;q=0.9,en;q=0.8',
            'Referer': 'https://winline.ru'
        }

        try:
            response = requests.get(url, headers=headers, timeout=30)
            response.raise_for_status()
            return response.text
        except Exception as e:
            logger.error(f"[Winline] Failed to fetch {url}: {e}")
            return ""

    def _extract_text_blocks(self, soup: BeautifulSoup) -> List[str]:
        """Extract text blocks that might contain events"""
        results = []

        selectors = [
            'div', 'span', 'p', 'section', 'article',
            '[class*="event"]', '[class*="match"]', '[class*="row"]',
            '[class*="card"]', '[class*="item"]'
        ]

        for selector in selectors:
            try:
                elements = soup.select(selector)
                for el in elements:
                    text = el.get_text().strip()
                    if (text and len(text) > 10 and
                        any(sep in text for sep in ['—', '-', 'vs', ':']) and
                        not any(re.search(pattern, text.lower()) for pattern in self.exclude_patterns)):
                        results.append(text)
            except:
                continue

        unique_results = list(set(results))
        return unique_results[:1000]

    def _parse_event_block(self, block: str) -> Dict:
        """Parse an event from a text block"""
        clean = re.sub(r'\s+\d+:\d+|\s+\d+\s*—\s*\d+', '', block)
        clean = re.sub(r'\s+', ' ', clean).strip()

        parts = None
        for sep in ['—', ':', '-', 'vs']:
            if sep in clean:
                parts = [p.strip() for p in clean.split(sep, 1)]
                break

        if not parts or len(parts) != 2:
            return None

        if len(parts[0]) < 4 or len(parts[1]) < 4:
            return None

        name = f"{parts[0]} — {parts[1]}"
        odds = [float(o.replace(',', '.')) for o in re.findall(r'(\d+[.,]\d+)', block)
                if 1.01 <= float(o.replace(',', '.')) <= 100.0]

        if len(odds) < 2:
            return None

        event = {
            'name': name,
            'normalized_name': normalize_event_name(name),
            'market_type': '1x2',
            'p1': odds[0],
            'p2': odds[-1],
            'bookmaker': 'Winline'
        }

        if any(x in block for x in ['2.5', '2,5']) and len(odds) >= 4:
            event_total = {
                'name': name,
                'normalized_name': normalize_event_name(name),
                'market_type': 'total',
                'total_value': 2.5,
                'over': odds[2],
                'under': odds[3],
                'bookmaker': 'Winline'
            }
            # Could return both, but for now just 1x2

        return event
