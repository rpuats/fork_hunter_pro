# scanner/parsers/winline_parser.py - HTTP-based HTML parsing
import logging
import re
import asyncio
from typing import List, Dict, Optional
import requests
from bs4 import BeautifulSoup
from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


class WinlineParser(BaseParser):
    name = "Winline"
    slug = "winline"
    base_url = "https://winline.ru"

    # Filter patterns
    exclude_patterns = [
        r'избранное', r'ближайшие', r'корзина', r'история', r'бонус',
        r'акция', r'кешбэк', r'генератор экспресса', r'размер коэффициента',
        r'сумма возм.выигрыша', r'только топ-события', r'добавить исход',
        r'обновить список', r'популярные события', r'добавить в корзину',
        r'подробнее', r'ежемесячно', r'деньгами', r'личный кабинет',
        r'пополнение', r'вывод', r'правила', r'помощь', r'поддержка'
    ]

    async def get_events(self) -> List[Dict]:
        events = []

        # Try API endpoints first
        api_events = await self._try_api_endpoints()
        if api_events:
            events.extend(api_events)
            logger.info(f"Winline: got {len(api_events)} events from API")
        else:
            # Fallback to HTML parsing
            urls_to_try = [
                "https://winline.ru/live",
                "https://winline.ru/line"
            ]

            for url in urls_to_try:
                try:
                    parsed_events = await self._scrape_url(url)
                    if parsed_events:
                        events.extend(parsed_events)
                        logger.info(f"Winline: got {len(parsed_events)} events from {url}")
                        break
                except Exception as e:
                    logger.debug(f"Winline: failed to scrape {url}: {e}")
                    continue

        logger.debug(f"Winline: total {len(events)} events collected")
        return events[:50]

    async def _try_api_endpoints(self) -> List[Dict]:
        """Try various API endpoints"""
        events = []

        api_patterns = [
            "https://winline.ru/api/events/live",
            "https://winline.ru/api/events/line",
            "https://winline.ru/api/v2/events/live",
            "https://winline.ru/api/v2/events/line",
            "https://winline.ru/api/betline/events",
            "https://winline.ru/api/betline/live",
            "https://winline.ru/api/betline/line",
        ]

        loop = asyncio.get_event_loop()

        for url in api_patterns:
            try:
                api_data = await loop.run_in_executor(None, self._fetch_api, url)
                if api_data:
                    parsed_events = self._parse_api_response(api_data)
                    if parsed_events:
                        events.extend(parsed_events)
                        break
            except Exception as e:
                logger.debug(f"Winline: API {url} failed: {e}")
                continue

        return events

    def _fetch_api(self, url: str):
        """Fetch API endpoint"""
        headers = {
            'Accept': 'application/json',
            'Referer': 'https://winline.ru',
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
        }

        try:
            response = requests.get(url, headers=headers, timeout=10)
            if response.status_code == 200:
                try:
                    return response.json()
                except:
                    return None
            return None
        except:
            return None

    def _parse_api_response(self, data) -> List[Dict]:
        """Parse API response"""
        events = []

        if not isinstance(data, (dict, list)):
            return events

        candidates = []
        if isinstance(data, dict):
            for key in ['events', 'data', 'matches', 'items']:
                if key in data and isinstance(data[key], list):
                    candidates.extend(data[key])
        elif isinstance(data, list):
            candidates = data

        for item in candidates:
            if isinstance(item, dict):
                event = self._parse_api_event(item)
                if event:
                    events.append(event)

        return events

    def _parse_api_event(self, item: dict) -> Optional[Dict]:
        """Parse single event from API"""
        home = item.get('home_team') or item.get('home') or item.get('team1')
        away = item.get('away_team') or item.get('away') or item.get('team2')

        if not home or not away:
            return None

        home_odds = item.get('home_odds') or item.get('k1') or item.get('win1')
        away_odds = item.get('away_odds') or item.get('k2') or item.get('win2')

        if not home_odds or not away_odds:
            return None

        try:
            home_odds = float(home_odds)
            away_odds = float(away_odds)
        except:
            return None

        if home_odds < 1.01 or away_odds < 1.01:
            return None

        name = f"{home} — {away}"

        return {
            'id': f"winline_{hash(name)}",
            'bookmaker': 'winline',
            'sport': 'football',
            'home_team': home,
            'away_team': away,
            'league': 'Live',
            'home_odds': home_odds,
            'draw_odds': None,
            'away_odds': away_odds,
            'is_live': True,
            'market': '1x2',
            'source_url': self.base_url
        }

    async def _scrape_url(self, url: str) -> List[Dict]:
        """Scrape events from URL using HTTP requests"""
        events = []

        # Run HTTP request in thread pool
        loop = asyncio.get_event_loop()
        html_content = await loop.run_in_executor(None, self._fetch_page, url)

        if not html_content:
            return events

        # Parse HTML
        soup = BeautifulSoup(html_content, 'html.parser')
        text_blocks = self._extract_text_blocks(soup)

        for block in text_blocks:
            try:
                event = self._parse_event_block(block)
                if event:
                    events.append(event)
            except Exception as e:
                logger.debug(f"Winline: failed to parse block: {e}")
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
            logger.error(f"Winline: failed to fetch {url}: {e}")
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

        # Remove duplicates and limit
        unique_results = list(set(results))
        return unique_results[:200]

    def _parse_event_block(self, block: str) -> Optional[Dict]:
        """Parse an event from a text block"""
        # Clean the block
        clean = re.sub(r'\s+\d+:\d+|\s+\d+\s*—\s*\d+', '', block)
        clean = re.sub(r'\s+', ' ', clean).strip()

        # Split into team names
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

        # Extract odds
        odds = [float(o.replace(',', '.')) for o in re.findall(r'(\d+[.,]\d+)', block)
                if 1.01 <= float(o.replace(',', '.')) <= 100.0]

        if len(odds) < 2:
            return None

        return {
            'id': f"winline_{hash(name)}",
            'bookmaker': 'winline',
            'sport': 'football',
            'home_team': parts[0],
            'away_team': parts[1],
            'league': 'Live' if 'live' in self.base_url else 'Pre-match',
            'home_odds': odds[0],
            'draw_odds': None,
            'away_odds': odds[-1],
            'is_live': 'live' in self.base_url,
            'market': '1x2',
            'source_url': self.base_url
        }
