# tests/test_connection_pool.py
import pytest
import asyncio
import sys
import os
from unittest.mock import AsyncMock, MagicMock, patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from core.connection_pool import SharedConnectionPool, OptimizedParserSession


class TestSharedConnectionPoolInit:
    def test_default_values(self):
        pool = SharedConnectionPool()
        assert pool.limit == 100
        assert pool.limit_per_host == 20
        assert pool.ttl_dns_cache == 300
        assert pool.use_dns_cache is True
        assert pool.keepalive_timeout == 30
        assert pool.enable_tcp_keepalive is True

    def test_custom_values(self):
        pool = SharedConnectionPool(
            limit=50,
            limit_per_host=10,
            ttl_dns_cache=60,
            use_dns_cache=False,
            keepalive_timeout=15,
            enable_tcp_keepalive=False,
        )
        assert pool.limit == 50
        assert pool.limit_per_host == 10
        assert pool.ttl_dns_cache == 60
        assert pool.use_dns_cache is False
        assert pool.keepalive_timeout == 15
        assert pool.enable_tcp_keepalive is False


class TestSharedConnectionPoolSingleton:
    @pytest.mark.asyncio
    async def test_get_instance_creates_pool(self):
        await SharedConnectionPool.reset()
        pool = await SharedConnectionPool.get_instance()
        assert pool is not None
        assert isinstance(pool, SharedConnectionPool)

    @pytest.mark.asyncio
    async def test_get_instance_returns_same_instance(self):
        await SharedConnectionPool.reset()
        pool1 = await SharedConnectionPool.get_instance()
        pool2 = await SharedConnectionPool.get_instance()
        assert pool1 is pool2

    @pytest.mark.asyncio
    async def test_reset_clears_instance(self):
        await SharedConnectionPool.reset()
        pool1 = await SharedConnectionPool.get_instance()
        await SharedConnectionPool.reset()
        pool2 = await SharedConnectionPool.get_instance()
        assert pool1 is not pool2

    @pytest.mark.asyncio
    async def test_singleton_with_custom_params(self):
        await SharedConnectionPool.reset()
        SharedConnectionPool._instance = SharedConnectionPool(limit=42)
        pool = await SharedConnectionPool.get_instance()
        assert pool.limit == 42

    @pytest.mark.asyncio
    async def teardown_method(self):
        await SharedConnectionPool.reset()


class TestSharedConnectionPoolSession:
    @pytest.mark.asyncio
    async def test_get_session_creates_session(self):
        pool = SharedConnectionPool()
        session = await pool.get_session()
        assert session is not None
        assert pool._session is not None

    @pytest.mark.asyncio
    async def test_get_session_reuses_session(self):
        pool = SharedConnectionPool()
        session1 = await pool.get_session()
        session2 = await pool.get_session()
        assert session1 is session2

    @pytest.mark.asyncio
    async def test_get_session_recreates_if_closed(self):
        pool = SharedConnectionPool()
        session1 = await pool.get_session()
        await pool.close()
        session2 = await pool.get_session()
        assert session2 is not None
        assert session2 is not session1


class TestSharedConnectionPoolClose:
    @pytest.mark.asyncio
    async def test_close_session(self):
        pool = SharedConnectionPool()
        await pool.get_session()
        await pool.close()
        assert pool._session is None

    @pytest.mark.asyncio
    async def test_close_without_session(self):
        pool = SharedConnectionPool()
        await pool.close()
        assert pool._session is None

    @pytest.mark.asyncio
    async def test_close_idempotent(self):
        pool = SharedConnectionPool()
        await pool.get_session()
        await pool.close()
        await pool.close()


class TestSharedConnectionPoolStats:
    @pytest.mark.asyncio
    async def test_stats_initial(self):
        pool = SharedConnectionPool()
        stats = pool.stats
        assert stats['total_requests'] == 0
        assert stats['total_errors'] == 0
        assert stats['error_rate'] == 0.0
        assert 'http2_available' in stats

    @pytest.mark.asyncio
    async def test_stats_after_requests(self):
        pool = SharedConnectionPool()
        pool.record_request()
        pool.record_request()
        stats = pool.stats
        assert stats['total_requests'] == 2

    @pytest.mark.asyncio
    async def test_stats_after_errors(self):
        pool = SharedConnectionPool()
        pool.record_request()
        pool.record_error()
        stats = pool.stats
        assert stats['total_errors'] == 1
        assert stats['error_rate'] > 0

    @pytest.mark.asyncio
    async def test_stats_error_rate_calculation(self):
        pool = SharedConnectionPool()
        for _ in range(10):
            pool.record_request()
        for _ in range(3):
            pool.record_error()
        stats = pool.stats
        assert stats['error_rate'] == 30.0

    @pytest.mark.asyncio
    async def test_stats_pool_age(self):
        pool = SharedConnectionPool()
        await pool.get_session()
        stats = pool.stats
        assert stats['pool_age_seconds'] >= 0


class TestOptimizedParserSession:
    @pytest.mark.asyncio
    async def test_get_session(self):
        pool = SharedConnectionPool()
        session_wrapper = OptimizedParserSession(pool)
        session = await session_wrapper.get_session()
        assert session is not None

    @pytest.mark.asyncio
    async def test_session_reuse(self):
        pool = SharedConnectionPool()
        session_wrapper = OptimizedParserSession(pool)
        s1 = await session_wrapper.get_session()
        s2 = await session_wrapper.get_session()
        assert s1 is s2

    @pytest.mark.asyncio
    async def test_close(self):
        pool = SharedConnectionPool()
        session_wrapper = OptimizedParserSession(pool)
        await session_wrapper.get_session()
        await session_wrapper.close()
        assert session_wrapper._session is None

    @pytest.mark.asyncio
    async def test_close_without_session(self):
        pool = SharedConnectionPool()
        session_wrapper = OptimizedParserSession(pool)
        await session_wrapper.close()
