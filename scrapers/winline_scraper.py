# scrapers/winline_scraper.py - HTTP-based parser (no Playwright)
import asyncio
import re
import logging
from typing import List, Dict
import requests
from bs4 import BeautifulSoup
from scrapers.base_scraper import BaseScraper
from core.event_normalizer import normalize_event_name

logger = logging.getLogger(__name__)

class WinlineScraper(BaseScraper):
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
            # Try API endpoints first
            api_events = await self._try_api_endpoints()
            if api_events:
                events.extend(api_events)
                logger.info(f"[Winline] Got {len(api_events)} events from API")
            else:
                # Fallback to HTML parsing
                urls = [
                    "https://winline.ru/live",
                    "https://winline.ru/line"
                ]

                for url in urls:
                    try:
                        events_from_url = await self._scrape_url(url)
                        if events_from_url:
                            events.extend(events_from_url)
                            logger.info(f"[Winline] Got {len(events_from_url)} events from {url}")
                            break
                    except Exception as e:
                        logger.debug(f"[Winline] Failed to scrape {url}: {e}")
                        continue

        except Exception as e:
            logger.error(f"[Winline] Error: {e}")

        logger.info(f"[Winline] Total {len(events)} events collected")
        return events

    async def _try_api_endpoints(self) -> List[Dict]:
        """Try various API endpoints to find working ones"""
        events = []

        # Known API patterns to try
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
                        break  # Found working API
            except Exception as e:
                logger.debug(f"[Winline] API {url} failed: {e}")
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
        """Parse API response for events"""
        events = []

        if not isinstance(data, (dict, list)):
            return events

        # Try different response structures
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

    def _parse_api_event(self, item: dict) -> Dict:
        """Parse a single event from API response"""
        # Try different field names
        home = item.get('home_team') or item.get('home') or item.get('team1') or item.get('homeTeam')
        away = item.get('away_team') or item.get('away') or item.get('team2') or item.get('awayTeam')

        if not home or not away:
            return None

        home_odds = item.get('home_odds') or item.get('k1') or item.get('win1') or item.get('coefficient1')
        away_odds = item.get('away_odds') or item.get('k2') or item.get('win2') or item.get('coefficient2')

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
            'name': name,
            'normalized_name': normalize_event_name(name),
            'market_type': '1x2',
            'p1': home_odds,
            'p2': away_odds,
            'bookmaker': 'Winline'
        }

    async def _scrape_url(self, url: str) -> List[Dict]:
        """Scrape events from a URL using HTTP requests"""
        events = []

        # Use asyncio to run requests in thread pool
        loop = asyncio.get_event_loop()
        response = await loop.run_in_executor(None, self._fetch_page, url)

        if not response:
            return events

        # Parse HTML
        soup = BeautifulSoup(response, 'html.parser')

        # Extract text blocks that might contain events
        raw_blocks = self._extract_text_blocks(soup)

        for block in raw_blocks:
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

        # Look for various selectors that might contain event data
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
                        not any(pattern in text.lower() for pattern in self.exclude_patterns)):
                        results.append(text)
            except:
                continue

        # Remove duplicates and limit
        unique_results = list(set(results))
        return unique_results[:200]

    def _parse_event_block(self, block: str) -> Dict:
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

        event = {
            'name': name,
            'normalized_name': normalize_event_name(name),
            'market_type': '1x2',
            'p1': odds[0],
            'p2': odds[-1],
            'bookmaker': 'Winline'
        }

        # Check for total markets
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
            # For now, return 1x2 event; totals would need separate handling
            pass

        return event
