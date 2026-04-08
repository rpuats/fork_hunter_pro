# scanner/parsers/base.py
import asyncio
import aiohttp
import time
import random
import logging
from abc import ABC, abstractmethod
from typing import List, Dict, Optional, Union, Any
from dataclasses import dataclass, field

from core.cache import rate_limiter
from core.connection_pool import SharedConnectionPool, connection_pool

logger = logging.getLogger(__name__)


@dataclass
class ParserConfig:
    name: str
    slug: str
    base_url: str
    rate_limit: float = 60
    timeout: float = 30.0
    max_retries: int = 3
    retry_delay: float = 1.0
    retry_backoff_factor: float = 2.0
    user_agents: List[str] = field(default_factory=lambda: [
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0',
        'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15',
        'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    ])


class ParserError(Exception):
    """Base exception for parser errors."""
    def __init__(self, message: str, slug: str = "", status_code: Optional[int] = None):
        super().__init__(message)
        self.slug = slug
        self.status_code = status_code


class ParserTimeoutError(ParserError):
    """Raised when parser request times out."""
    pass


class ParserRateLimitError(ParserError):
    """Raised when parser hits rate limit."""
    pass


class ParserConnectionError(ParserError):
    """Raised when parser cannot connect."""
    pass


class BaseParser(ABC):
    name: str = "base"
    slug: str = "base"
    base_url: str = ""
    
    def __init__(self, config: Optional[ParserConfig] = None):
        self.config = config or self._default_config()
        self.session: Optional[aiohttp.ClientSession] = None
        self._last_request = 0
        self._request_count = 0
        self._errors = 0
        self._success_count = 0
        self._last_error: Optional[str] = None
        self._last_success_time: Optional[float] = None
    
    def _default_config(self) -> ParserConfig:
        return ParserConfig(
            name=self.name,
            slug=self.slug,
            base_url=self.base_url,
            rate_limit=60,
            timeout=30.0,
            max_retries=3,
            retry_delay=1.0,
            user_agents=[
                'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0',
                'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15',
                'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
            ]
        )
    
    async def get_session(self) -> aiohttp.ClientSession:
        pool = await SharedConnectionPool.get_instance()
        return await pool.get_session()
    
    async def close(self):
        pool = await SharedConnectionPool.get_instance()
        await pool.close()
        logger.debug(f"Closed session for {self.name}")
    
    def _get_headers(self) -> Dict[str, str]:
        ua = random.choice(self.config.user_agents)
        return {
            "User-Agent": ua,
            "Accept": "application/json, text/plain, */*",
            "Accept-Language": "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7",
            "Accept-Encoding": "gzip, deflate, br",
            "Connection": "keep-alive",
            "Cache-Control": "no-cache",
        }
    
    async def _wait_for_rate_limit(self):
        await rate_limiter.wait_and_acquire(self.slug, tokens=1.0, timeout=10.0)
        
        min_interval = 60.0 / self.config.rate_limit
        elapsed = time.time() - self._last_request
        if elapsed < min_interval:
            await asyncio.sleep(min_interval - elapsed)
    
    def _calculate_backoff_delay(self, attempt: int) -> float:
        """Calculate exponential backoff delay with jitter."""
        base_delay = self.config.retry_delay * (self.config.retry_backoff_factor ** attempt)
        jitter = random.uniform(0, base_delay * 0.1)
        return min(base_delay + jitter, 30.0)
    
    async def fetch(self, url: str, method: str = 'GET', json: bool = True, **kwargs) -> Optional[Any]:
        await self._wait_for_rate_limit()
        
        session = await self.get_session()
        headers = self._get_headers()
        headers.update(kwargs.pop('headers', {}))
        
        for attempt in range(self.config.max_retries):
            try:
                async with session.request(
                    method,
                    url,
                    headers=headers,
                    **kwargs
                ) as resp:
                    self._last_request = time.time()
                    self._request_count += 1
                    
                    if resp.status == 200:
                        self._success_count += 1
                        self._last_success_time = time.time()
                        if json:
                            return await resp.json()
                        return await resp.text()
                    elif resp.status == 429:
                        delay = self._calculate_backoff_delay(attempt)
                        logger.warning(f"{self.name}: rate limited on {url}, retrying in {delay:.1f}s (attempt {attempt + 1})")
                        await asyncio.sleep(delay)
                        continue
                    elif resp.status == 403:
                        self._errors += 1
                        self._last_error = f"403 Forbidden"
                        logger.warning(f"{self.name}: 403 Forbidden on {url}")
                        return None
                    elif resp.status >= 500:
                        delay = self._calculate_backoff_delay(attempt)
                        logger.warning(f"{self.name}: server error {resp.status} on {url}, retrying in {delay:.1f}s")
                        await asyncio.sleep(delay)
                        continue
                    else:
                        logger.debug(f"{self.name}: unexpected status {resp.status} on {url}")
                        return None
                        
            except asyncio.TimeoutError:
                self._errors += 1
                self._last_error = f"Timeout on attempt {attempt + 1}"
                if attempt < self.config.max_retries - 1:
                    delay = self._calculate_backoff_delay(attempt)
                    logger.warning(f"{self.name}: timeout on {url}, retrying in {delay:.1f}s (attempt {attempt + 1})")
                    await asyncio.sleep(delay)
                    continue
                logger.error(f"{self.name}: timeout after {self.config.max_retries} attempts on {url}")
                return None
            except aiohttp.ClientError as e:
                self._errors += 1
                self._last_error = f"ClientError: {str(e)}"
                if attempt < self.config.max_retries - 1:
                    delay = self._calculate_backoff_delay(attempt)
                    logger.warning(f"{self.name}: connection error on {url}, retrying in {delay:.1f}s: {e}")
                    await asyncio.sleep(delay)
                    continue
                logger.error(f"{self.name}: connection error after {self.config.max_retries} attempts on {url}: {e}")
                return None
            except Exception as e:
                self._errors += 1
                self._last_error = f"Unexpected error: {str(e)}"
                if attempt < self.config.max_retries - 1:
                    delay = self._calculate_backoff_delay(attempt)
                    logger.warning(f"{self.name}: unexpected error on {url}, retrying in {delay:.1f}s: {e}")
                    await asyncio.sleep(delay)
                    continue
                logger.error(f"{self.name}: unexpected error after {self.config.max_retries} attempts on {url}: {e}")
                return None
        
        return None
    
    async def fetch_with_fallback(self, urls: List[str]) -> Optional[Any]:
        for url in urls:
            data = await self.fetch(url)
            if data:
                return data
        return None
    
    def get_stats(self) -> Dict:
        total = self._request_count
        success = self._success_count
        errors = self._errors
        return {
            'name': self.name,
            'slug': self.slug,
            'requests': total,
            'successes': success,
            'errors': errors,
            'error_rate': round(errors / max(total, 1) * 100, 2),
            'success_rate': round(success / max(total, 1) * 100, 2),
            'last_error': self._last_error,
            'last_success_time': self._last_success_time,
        }
    
    @abstractmethod
    async def get_events(self) -> List[Dict]:
        pass
    
    def _normalize_event(self, raw: Dict) -> Optional[Dict]:
        try:
            home = raw.get('home_team') or raw.get('home') or raw.get('team1') or ''
            away = raw.get('away_team') or raw.get('away') or raw.get('team2') or ''
            
            if not home or not away:
                return None
            
            home_odds = float(raw.get('home_odds') or raw.get('k1') or raw.get('win1') or raw.get('coefficient1') or 0)
            draw_odds = float(raw.get('draw_odds') or raw.get('kx') or raw.get('coefficientX') or 0)
            away_odds = float(raw.get('away_odds') or raw.get('k2') or raw.get('win2') or raw.get('coefficient2') or 0)
            
            if home_odds < 1.01 and away_odds < 1.01:
                return None
            
            return {
                'id': raw.get('id', f"{self.slug}_{hash(home + away)}"),
                'bookmaker': self.slug,
                'sport': raw.get('sport', 'football'),
                'home_team': home,
                'away_team': away,
                'league': raw.get('league') or raw.get('champ') or 'Live',
                'home_odds': home_odds,
                'draw_odds': draw_odds if draw_odds > 1.0 else None,
                'away_odds': away_odds,
                'is_live': raw.get('is_live', True),
                'market': '1x2',
                'source_url': self.base_url,
                'scraped_at': time.time()
            }
        except Exception as e:
            logger.debug(f"{self.name}: error normalizing event: {e}")
            return None

    def _extract_odds(self, coeffs: Any, keys: List[str]) -> float:
        """Extract odds value from dict by trying multiple keys."""
        if not isinstance(coeffs, dict):
            return 0.0
        for key in keys:
            val = coeffs.get(key)
            if val is not None:
                try:
                    return float(val)
                except (ValueError, TypeError):
                    continue
        return 0.0
