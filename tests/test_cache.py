# tests/test_cache.py
import pytest
import asyncio
import time
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.cache import TTLCache, AsyncTTLCache, RateLimiter, MultiLimiter


class TestTTLCache:
    def test_basic_set_get(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        cache.set('key1', 'value1')
        assert cache.get('key1') == 'value1'

    def test_get_missing_key(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        assert cache.get('missing') is None
        assert cache.get('missing', 'default') == 'default'

    def test_ttl_expiration(self):
        cache = TTLCache(maxsize=100, default_ttl=0.1)
        cache.set('key1', 'value1')
        assert cache.get('key1') == 'value1'
        time.sleep(0.15)
        assert cache.get('key1') is None

    def test_delete(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        cache.set('key1', 'value1')
        cache.delete('key1')
        assert cache.get('key1') is None

    def test_clear(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        cache.set('key1', 'value1')
        cache.set('key2', 'value2')
        cache.clear()
        assert cache.get('key1') is None
        assert cache.get('key2') is None

    def test_maxsize_eviction(self):
        cache = TTLCache(maxsize=2, default_ttl=30.0)
        cache.set('key1', 'value1')
        cache.set('key2', 'value2')
        cache.set('key3', 'value3')
        assert cache.get('key1') is None
        assert cache.get('key2') == 'value2'
        assert cache.get('key3') == 'value3'

    def test_stats(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        cache.set('key1', 'value1')
        cache.get('key1')
        cache.get('missing')
        stats = cache.stats()
        assert stats['hits'] == 1
        assert stats['misses'] == 1
        assert stats['size'] == 1
        assert stats['hit_rate'] == 50.0

    def test_stats_empty_cache(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        stats = cache.stats()
        assert stats['hit_rate'] == 0

    def test_cleanup_expired(self):
        cache = TTLCache(maxsize=100, default_ttl=0.1)
        cache.set('key1', 'value1')
        cache.set('key2', 'value2')
        time.sleep(0.15)
        cleaned = cache.cleanup_expired()
        assert cleaned == 2
        assert cache.stats()['size'] == 0

    def test_custom_ttl_per_key(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        cache.set('key1', 'value1', ttl=0.1)
        cache.set('key2', 'value2', ttl=30.0)
        time.sleep(0.15)
        assert cache.get('key1') is None
        assert cache.get('key2') == 'value2'

    def test_access_count(self):
        cache = TTLCache(maxsize=100, default_ttl=30.0)
        cache.set('key1', 'value1')
        cache.get('key1')
        cache.get('key1')
        entry = cache._cache['key1']
        assert entry.access_count == 2


class TestAsyncTTLCache:
    @pytest.mark.asyncio
    async def test_async_set_get(self):
        cache = AsyncTTLCache(maxsize=100, default_ttl=30.0)
        await cache.set('key1', 'value1')
        result = await cache.get('key1')
        assert result == 'value1'

    @pytest.mark.asyncio
    async def test_async_get_missing(self):
        cache = AsyncTTLCache(maxsize=100, default_ttl=30.0)
        result = await cache.get('missing')
        assert result is None

    @pytest.mark.asyncio
    async def test_async_delete(self):
        cache = AsyncTTLCache(maxsize=100, default_ttl=30.0)
        await cache.set('key1', 'value1')
        await cache.delete('key1')
        result = await cache.get('key1')
        assert result is None

    @pytest.mark.asyncio
    async def test_async_clear(self):
        cache = AsyncTTLCache(maxsize=100, default_ttl=30.0)
        await cache.set('key1', 'value1')
        await cache.set('key2', 'value2')
        await cache.clear()
        assert await cache.get('key1') is None
        assert await cache.get('key2') is None

    @pytest.mark.asyncio
    async def test_async_ttl_expiration(self):
        cache = AsyncTTLCache(maxsize=100, default_ttl=0.1)
        await cache.set('key1', 'value1')
        assert await cache.get('key1') == 'value1'
        await asyncio.sleep(0.15)
        assert await cache.get('key1') is None

    @pytest.mark.asyncio
    async def test_async_cleanup_expired(self):
        cache = AsyncTTLCache(maxsize=100, default_ttl=0.1)
        await cache.set('key1', 'value1')
        await asyncio.sleep(0.15)
        cleaned = await cache.cleanup_expired()
        assert cleaned == 1

    def test_async_stats(self):
        cache = AsyncTTLCache(maxsize=100, default_ttl=30.0)
        stats = cache.stats()
        assert 'size' in stats
        assert 'hits' in stats
        assert 'misses' in stats


class TestRateLimiter:
    def test_acquire_within_limit(self):
        limiter = RateLimiter(rate=10, per=1.0, burst=10)
        assert limiter.acquire(1.0) is True

    def test_acquire_exceeds_burst(self):
        limiter = RateLimiter(rate=10, per=1.0, burst=2)
        assert limiter.acquire(1.0) is True
        assert limiter.acquire(1.0) is True
        assert limiter.acquire(1.0) is False

    def test_wait_time(self):
        limiter = RateLimiter(rate=10, per=1.0, burst=1)
        limiter.acquire(1.0)
        wait = limiter.wait_time(1.0)
        assert wait > 0

    def test_wait_time_no_wait(self):
        limiter = RateLimiter(rate=10, per=1.0, burst=10)
        wait = limiter.wait_time(1.0)
        assert wait == 0.0

    @pytest.mark.asyncio
    async def test_acquire_async(self):
        limiter = RateLimiter(rate=10, per=1.0, burst=10)
        result = await limiter.acquire_async(1.0)
        assert result is True

    def test_token_regeneration(self):
        limiter = RateLimiter(rate=100, per=1.0, burst=1)
        limiter.acquire(1.0)
        time.sleep(0.05)
        assert limiter.acquire(1.0) is True


class TestMultiLimiter:
    def test_get_limiter(self):
        ml = MultiLimiter()
        limiter1 = ml.get_limiter('bk1', rate=10)
        limiter2 = ml.get_limiter('bk1', rate=10)
        assert limiter1 is limiter2

    def test_acquire(self):
        ml = MultiLimiter()
        assert ml.acquire('bk1', 1.0) is True

    @pytest.mark.asyncio
    async def test_wait_and_acquire(self):
        ml = MultiLimiter()
        result = await ml.wait_and_acquire('bk1', 1.0, timeout=1.0)
        assert result is True

    @pytest.mark.asyncio
    async def test_wait_and_acquire_timeout(self):
        ml = MultiLimiter()
        limiter = ml.get_limiter('bk2', rate=0.1)
        limiter.acquire(1.0)
        result = await ml.wait_and_acquire('bk2', 1.0, timeout=0.1)
        assert result is False
