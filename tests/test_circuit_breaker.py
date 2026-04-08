# tests/test_circuit_breaker.py
import pytest
import asyncio
import time
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from scanner.parsers.circuit_breaker import (
    CircuitBreaker,
    CircuitState,
    CircuitBreakerOpenError,
    CircuitBreakerStats,
)


async def success_func(*args, **kwargs):
    return "ok"


async def fail_func(*args, **kwargs):
    raise RuntimeError("fail")


def sync_success_func(*args, **kwargs):
    return "sync_ok"


def sync_fail_func(*args, **kwargs):
    raise RuntimeError("sync_fail")


class TestCircuitBreakerInit:
    def test_default_values(self):
        cb = CircuitBreaker(slug="test")
        assert cb.slug == "test"
        assert cb.failure_threshold == 5
        assert cb.recovery_timeout == 60.0
        assert cb.half_open_max_calls == 1
        assert cb.success_threshold == 2
        assert cb.state == CircuitState.CLOSED

    def test_custom_values(self):
        cb = CircuitBreaker(
            slug="custom",
            failure_threshold=3,
            recovery_timeout=30.0,
            half_open_max_calls=2,
            success_threshold=1,
        )
        assert cb.failure_threshold == 3
        assert cb.recovery_timeout == 30.0
        assert cb.half_open_max_calls == 2
        assert cb.success_threshold == 1


class TestCircuitBreakerClosed:
    @pytest.mark.asyncio
    async def test_success_passes_through(self):
        cb = CircuitBreaker(slug="test")
        result = await cb.call(success_func)
        assert result == "ok"
        assert cb.state == CircuitState.CLOSED

    @pytest.mark.asyncio
    async def test_sync_success_passes_through(self):
        cb = CircuitBreaker(slug="test")
        result = await cb.call(sync_success_func)
        assert result == "sync_ok"

    @pytest.mark.asyncio
    async def test_failure_records(self):
        cb = CircuitBreaker(slug="test")
        with pytest.raises(RuntimeError, match="fail"):
            await cb.call(fail_func)
        assert cb.stats.failed_calls == 1
        assert cb.stats.total_calls == 1


class TestCircuitBreakerOpens:
    @pytest.mark.asyncio
    async def test_opens_after_threshold(self):
        cb = CircuitBreaker(slug="test", failure_threshold=3)
        for _ in range(3):
            with pytest.raises(RuntimeError):
                await cb.call(fail_func)
        assert cb.state == CircuitState.OPEN

    @pytest.mark.asyncio
    async def test_open_rejects_calls(self):
        cb = CircuitBreaker(slug="test", failure_threshold=1)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        assert cb.state == CircuitState.OPEN
        with pytest.raises(CircuitBreakerOpenError):
            await cb.call(success_func)

    @pytest.mark.asyncio
    async def test_open_error_message_includes_slug(self):
        cb = CircuitBreaker(slug="myservice", failure_threshold=1)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        with pytest.raises(CircuitBreakerOpenError, match="myservice"):
            await cb.call(success_func)


class TestCircuitBreakerHalfOpen:
    @pytest.mark.asyncio
    async def test_transitions_to_half_open_after_timeout(self):
        cb = CircuitBreaker(slug="test", failure_threshold=1, recovery_timeout=0.01)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        assert cb.state == CircuitState.OPEN
        await asyncio.sleep(0.02)
        result = await cb.call(success_func)
        assert result == "ok"
        assert cb.state == CircuitState.HALF_OPEN

    @pytest.mark.asyncio
    async def test_half_open_success_closes_after_threshold(self):
        cb = CircuitBreaker(
            slug="test",
            failure_threshold=1,
            recovery_timeout=0.01,
            success_threshold=1,
        )
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        await asyncio.sleep(0.02)
        await cb.call(success_func)
        assert cb.state == CircuitState.CLOSED

    @pytest.mark.asyncio
    async def test_half_open_failure_reopens(self):
        cb = CircuitBreaker(
            slug="test",
            failure_threshold=1,
            recovery_timeout=0.01,
            success_threshold=2,
        )
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        await asyncio.sleep(0.02)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        assert cb.state == CircuitState.OPEN

    @pytest.mark.asyncio
    async def test_half_open_max_calls_rejected(self):
        cb = CircuitBreaker(
            slug="test",
            failure_threshold=1,
            recovery_timeout=0.01,
            half_open_max_calls=1,
            success_threshold=5,
        )
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        await asyncio.sleep(0.02)
        await cb.call(success_func)
        with pytest.raises(CircuitBreakerOpenError, match="HALF_OPEN"):
            await cb.call(success_func)


class TestCircuitBreakerReset:
    @pytest.mark.asyncio
    async def test_manual_reset(self):
        cb = CircuitBreaker(slug="test", failure_threshold=1)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        assert cb.state == CircuitState.OPEN
        cb.reset()
        assert cb.state == CircuitState.CLOSED
        assert cb.stats.consecutive_failures == 0
        assert cb.stats.consecutive_successes == 0

    @pytest.mark.asyncio
    async def test_reset_allows_calls_again(self):
        cb = CircuitBreaker(slug="test", failure_threshold=1)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        cb.reset()
        result = await cb.call(success_func)
        assert result == "ok"


class TestCircuitBreakerStats:
    @pytest.mark.asyncio
    async def test_success_rate(self):
        cb = CircuitBreaker(slug="test")
        assert cb.success_rate == 100.0
        await cb.call(success_func)
        await cb.call(success_func)
        assert cb.success_rate == 100.0

    @pytest.mark.asyncio
    async def test_success_rate_with_failures(self):
        cb = CircuitBreaker(slug="test", failure_threshold=10)
        await cb.call(success_func)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        assert cb.success_rate == 50.0

    @pytest.mark.asyncio
    async def test_get_status(self):
        cb = CircuitBreaker(slug="test")
        await cb.call(success_func)
        status = cb.get_status()
        assert status["slug"] == "test"
        assert status["state"] == "closed"
        assert status["total_calls"] == 1
        assert status["successful_calls"] == 1
        assert status["failed_calls"] == 0
        assert "success_rate" in status
        assert "recovery_timeout" in status

    @pytest.mark.asyncio
    async def test_consecutive_counters(self):
        cb = CircuitBreaker(slug="test", failure_threshold=10)
        await cb.call(success_func)
        await cb.call(success_func)
        assert cb.stats.consecutive_successes == 2
        assert cb.stats.consecutive_failures == 0
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        assert cb.stats.consecutive_failures == 1
        assert cb.stats.consecutive_successes == 0


class TestCircuitBreakerEdgeCases:
    def test_time_until_reset_no_failures(self):
        cb = CircuitBreaker(slug="test")
        assert cb._time_until_reset() == 0

    @pytest.mark.asyncio
    async def test_should_attempt_reset_no_failures(self):
        cb = CircuitBreaker(slug="test")
        assert cb._should_attempt_reset() is True

    @pytest.mark.asyncio
    async def test_stats_last_failure_time_set(self):
        cb = CircuitBreaker(slug="test", failure_threshold=10)
        with pytest.raises(RuntimeError):
            await cb.call(fail_func)
        assert cb.stats.last_failure_time is not None
        assert cb.stats.last_success_time is None

    @pytest.mark.asyncio
    async def test_stats_last_success_time_set(self):
        cb = CircuitBreaker(slug="test")
        await cb.call(success_func)
        assert cb.stats.last_success_time is not None
        assert cb.stats.last_failure_time is None
