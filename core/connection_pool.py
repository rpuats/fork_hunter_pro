# core/connection_pool.py
"""
Optimized connection pooling for aiohttp with:
- Shared connection pool across all parsers
- DNS caching
- TCP keep-alive
- HTTP/2 support (if aiohttp[speedups] installed)
"""
import asyncio
import time
import logging
from typing import Optional, Dict
import aiohttp

logger = logging.getLogger(__name__)

try:
    import aiohttp.http
    HAS_HTTP2 = hasattr(aiohttp.http, 'HttpVersion20')
except (ImportError, AttributeError):
    HAS_HTTP2 = False


class SharedConnectionPool:
    """
    Singleton connection pool shared across all parsers.
    Reduces connection overhead and enables connection reuse.
    """

    _instance: Optional['SharedConnectionPool'] = None
    _lock = asyncio.Lock()

    def __init__(
        self,
        limit: int = 100,
        limit_per_host: int = 20,
        ttl_dns_cache: int = 300,
        use_dns_cache: bool = True,
        keepalive_timeout: int = 30,
        enable_tcp_keepalive: bool = True,
    ):
        self.limit = limit
        self.limit_per_host = limit_per_host
        self.ttl_dns_cache = ttl_dns_cache
        self.use_dns_cache = use_dns_cache
        self.keepalive_timeout = keepalive_timeout
        self.enable_tcp_keepalive = enable_tcp_keepalive

        self._connector: Optional[aiohttp.TCPConnector] = None
        self._session: Optional[aiohttp.ClientSession] = None
        self._created_at = 0
        self._request_count = 0
        self._errors = 0

    @classmethod
    async def get_instance(cls) -> 'SharedConnectionPool':
        if cls._instance is None:
            async with cls._lock:
                if cls._instance is None:
                    cls._instance = cls()
        return cls._instance

    @classmethod
    async def reset(cls):
        async with cls._lock:
            if cls._instance:
                await cls._instance.close()
                cls._instance = None

    async def _create_connector(self) -> aiohttp.TCPConnector:
        connector_kwargs = {
            'limit': self.limit,
            'limit_per_host': self.limit_per_host,
            'ttl_dns_cache': self.ttl_dns_cache,
            'use_dns_cache': self.use_dns_cache,
            'keepalive_timeout': self.keepalive_timeout,
            'enable_cleanup_closed': True,
            'force_close': False,
        }

        if self.enable_tcp_keepalive:
            try:
                connector_kwargs['enable_tcp_keepalive'] = True
                aiohttp.TCPConnector(**connector_kwargs)
            except TypeError:
                connector_kwargs.pop('enable_tcp_keepalive', None)

        return aiohttp.TCPConnector(**connector_kwargs)

    async def get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            connector = await self._create_connector()

            timeout = aiohttp.ClientTimeout(
                total=30,
                connect=10,
                sock_read=15,
            )

            session_kwargs = {
                'connector': connector,
                'timeout': timeout,
                'trust_env': False,
            }

            self._session = aiohttp.ClientSession(**session_kwargs)
            self._connector = connector
            self._created_at = time.time()

        return self._session

    async def close(self):
        if self._session and not self._session.closed:
            await self._session.close()
            logger.info(f"Connection pool closed. Total requests: {self._request_count}, Errors: {self._errors}")
        if self._connector and not self._connector.closed:
            await self._connector.close()
        self._session = None
        self._connector = None

    def record_request(self):
        self._request_count += 1

    def record_error(self):
        self._errors += 1

    @property
    def stats(self) -> Dict:
        connector_stats = {}
        if self._connector and not self._connector.closed:
            connector_stats = {
                'open_connections': len(self._connector._acquired) if hasattr(self._connector, '_acquired') else 0,
                'available_connections': self._connector.limit - len(self._connector._acquired) if hasattr(self._connector, '_acquired') else self.limit,
            }

        return {
            'pool_age_seconds': round(time.time() - self._created_at, 1) if self._created_at else 0,
            'total_requests': self._request_count,
            'total_errors': self._errors,
            'error_rate': round(self._errors / max(self._request_count, 1) * 100, 2),
            'http2_available': HAS_HTTP2,
            **connector_stats,
        }


class OptimizedParserSession:
    """
    Wrapper that gives each parser its own session but backed by shared connector.
    """

    def __init__(self, pool: SharedConnectionPool):
        self._pool = pool
        self._session: Optional[aiohttp.ClientSession] = None

    async def get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            connector = await self._pool._create_connector()
            timeout = aiohttp.ClientTimeout(total=30, connect=10, sock_read=15)
            self._session = aiohttp.ClientSession(connector=connector, timeout=timeout)
        return self._session

    async def close(self):
        if self._session and not self._session.closed:
            await self._session.close()
        self._session = None


connection_pool = SharedConnectionPool()
