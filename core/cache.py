# core/cache.py
import asyncio
import time
from typing import Any, Dict, Optional, Callable
from dataclasses import dataclass, field
from collections import OrderedDict
import threading
import logging

logger = logging.getLogger(__name__)


@dataclass
class CacheEntry:
    value: Any
    created_at: float
    ttl: float
    access_count: int = 0
    
    def is_expired(self) -> bool:
        if self.ttl <= 0:
            return False
        return time.time() - self.created_at > self.ttl


class TTLCache:
    """Thread-safe LRU cache with TTL support"""
    
    def __init__(self, maxsize: int = 10000, default_ttl: float = 30.0):
        self.maxsize = maxsize
        self.default_ttl = default_ttl
        self._cache: OrderedDict[str, CacheEntry] = OrderedDict()
        self._lock = threading.RLock()
        self._hits = 0
        self._misses = 0
    
    def get(self, key: str, default: Any = None) -> Any:
        with self._lock:
            entry = self._cache.get(key)
            
            if entry is None:
                self._misses += 1
                return default
            
            if entry.is_expired():
                del self._cache[key]
                self._misses += 1
                return default
            
            entry.access_count += 1
            self._cache.move_to_end(key)
            self._hits += 1
            return entry.value
    
    def set(self, key: str, value: Any, ttl: Optional[float] = None):
        with self._lock:
            if len(self._cache) >= self.maxsize:
                self._cache.popitem(last=False)
            
            self._cache[key] = CacheEntry(
                value=value,
                created_at=time.time(),
                ttl=ttl if ttl is not None else self.default_ttl
            )
            self._cache.move_to_end(key)
    
    def delete(self, key: str):
        with self._lock:
            self._cache.pop(key, None)
    
    def clear(self):
        with self._lock:
            self._cache.clear()
            self._hits = 0
            self._misses = 0
    
    def cleanup_expired(self):
        with self._lock:
            expired = [
                k for k, v in self._cache.items()
                if v.is_expired()
            ]
            for k in expired:
                del self._cache[k]
            return len(expired)
    
    def stats(self) -> Dict:
        with self._lock:
            total = self._hits + self._misses
            hit_rate = (self._hits / total * 100) if total > 0 else 0
            return {
                'size': len(self._cache),
                'maxsize': self.maxsize,
                'hits': self._hits,
                'misses': self._misses,
                'hit_rate': round(hit_rate, 2)
            }


class AsyncTTLCache:
    """Async version of TTLCache with asyncio.Lock"""
    
    def __init__(self, maxsize: int = 10000, default_ttl: float = 30.0):
        self.cache = TTLCache(maxsize, default_ttl)
        self._lock = asyncio.Lock()
    
    async def get(self, key: str, default: Any = None) -> Any:
        async with self._lock:
            return self.cache.get(key, default)
    
    async def set(self, key: str, value: Any, ttl: Optional[float] = None):
        async with self._lock:
            self.cache.set(key, value, ttl)
    
    async def delete(self, key: str):
        async with self._lock:
            self.cache.delete(key)
    
    async def clear(self):
        async with self._lock:
            self.cache.clear()
    
    async def cleanup_expired(self):
        async with self._lock:
            return self.cache.cleanup_expired()
    
    def stats(self) -> Dict:
        return self.cache.stats()


class RateLimiter:
    """Token bucket rate limiter"""
    
    def __init__(self, rate: float, per: float = 1.0, burst: Optional[float] = None):
        self.rate = rate
        self.per = per
        self.burst = burst or rate
        self._tokens = self.burst
        self._last_update = time.time()
        self._lock = threading.Lock()
    
    def acquire(self, tokens: float = 1.0) -> bool:
        with self._lock:
            now = time.time()
            elapsed = now - self._last_update
            self._tokens = min(self.burst, self._tokens + elapsed * self.rate)
            self._last_update = now
            
            if self._tokens >= tokens:
                self._tokens -= tokens
                return True
            return False
    
    async def acquire_async(self, tokens: float = 1.0) -> bool:
        return self.acquire(tokens)
    
    def wait_time(self, tokens: float = 1.0) -> float:
        with self._lock:
            needed = tokens - self._tokens
            if needed <= 0:
                return 0.0
            return needed / self.rate


class MultiLimiter:
    """Rate limiter for multiple sources"""
    
    def __init__(self):
        self._limiters: Dict[str, RateLimiter] = {}
        self._lock = threading.Lock()
    
    def get_limiter(self, key: str, rate: float, per: float = 60.0) -> RateLimiter:
        with self._lock:
            if key not in self._limiters:
                self._limiters[key] = RateLimiter(rate, per)
            return self._limiters[key]
    
    def acquire(self, key: str, tokens: float = 1.0) -> bool:
        limiter = self.get_limiter(key, 60)
        return limiter.acquire(tokens)
    
    async def wait_and_acquire(self, key: str, tokens: float = 1.0, timeout: float = 10.0):
        limiter = self.get_limiter(key, 60)
        start = time.time()
        
        while time.time() - start < timeout:
            if limiter.acquire(tokens):
                return True
            wait = limiter.wait_time(tokens)
            if wait > 0:
                await asyncio.sleep(min(wait, timeout - (time.time() - start)))
        
        return False


event_cache = AsyncTTLCache(maxsize=50000, default_ttl=10.0)
surebet_cache = AsyncTTLCache(maxsize=1000, default_ttl=30.0)
rate_limiter = MultiLimiter()
