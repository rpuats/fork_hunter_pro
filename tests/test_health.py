# tests/test_health.py
import pytest
import asyncio
import time
import sys
import os
from unittest.mock import AsyncMock, MagicMock, patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from scanner.parsers.health import ParserHealthChecker, ParserHealthStatus
from scanner.parsers.circuit_breaker import CircuitBreakerOpenError


class MockParser:
    def __init__(self, slug="mock", name="Mock", events=None, raise_on_call=None):
        self.slug = slug
        self.name = name
        self._events = events or []
        self._raise_on_call = raise_on_call

    async def get_events(self):
        if self._raise_on_call:
            raise self._raise_on_call
        return self._events


class TestParserHealthStatus:
    def test_default_values(self):
        status = ParserHealthStatus(slug="test", name="Test")
        assert status.is_enabled is True
        assert status.is_healthy is True
        assert status.success_rate == 100.0
        assert status.total_checks == 0
        assert status.failed_checks == 0
        assert status.consecutive_failures == 0

    def test_custom_values(self):
        status = ParserHealthStatus(
            slug="test",
            name="Test",
            is_enabled=False,
            is_healthy=False,
            failed_checks=5,
        )
        assert status.is_enabled is False
        assert status.is_healthy is False
        assert status.failed_checks == 5


class TestParserHealthCheckerInit:
    def test_default_values(self):
        checker = ParserHealthChecker()
        assert checker.failure_threshold == 5
        assert checker.cooldown_seconds == 300.0
        assert checker.check_timeout == 15.0

    def test_custom_values(self):
        checker = ParserHealthChecker(
            failure_threshold=3,
            cooldown_seconds=60.0,
            check_timeout=5.0,
        )
        assert checker.failure_threshold == 3
        assert checker.cooldown_seconds == 60.0
        assert checker.check_timeout == 5.0


class TestParserHealthCheckerRegister:
    def test_register_parser(self):
        checker = ParserHealthChecker()
        parser = MockParser(slug="test", name="Test")
        checker.register_parser(parser)
        assert "test" in checker.statuses
        assert "test" in checker._parsers

    def test_register_multiple_parsers(self):
        checker = ParserHealthChecker()
        checker.register_parser(MockParser(slug="a", name="A"))
        checker.register_parser(MockParser(slug="b", name="B"))
        assert len(checker.statuses) == 2


class TestParserHealthCheckerCheck:
    @pytest.mark.asyncio
    async def test_check_success(self):
        checker = ParserHealthChecker()
        parser = MockParser(slug="test", name="Test", events=[{"id": 1}])
        checker.register_parser(parser)
        status = await checker.check_parser("test")
        assert status.is_healthy is True
        assert status.total_checks == 1
        assert status.failed_checks == 0
        assert status.consecutive_failures == 0
        assert status.last_success_time is not None

    @pytest.mark.asyncio
    async def test_check_failure(self):
        checker = ParserHealthChecker()
        parser = MockParser(
            slug="test",
            name="Test",
            raise_on_call=RuntimeError("connection error"),
        )
        checker.register_parser(parser)
        status = await checker.check_parser("test")
        assert status.is_healthy is False
        assert status.total_checks == 1
        assert status.failed_checks == 1
        assert status.consecutive_failures == 1

    @pytest.mark.asyncio
    async def test_check_timeout(self):
        checker = ParserHealthChecker(check_timeout=0.01)

        class SlowParser:
            slug = "slow"
            name = "Slow"

            async def get_events(self):
                await asyncio.sleep(10)
                return []

        checker.register_parser(SlowParser())
        status = await checker.check_parser("slow")
        assert status.is_healthy is False
        assert "Timeout" in status.last_error

    @pytest.mark.asyncio
    async def test_check_circuit_breaker_open(self):
        checker = ParserHealthChecker()
        parser = MockParser(
            slug="test",
            name="Test",
            raise_on_call=CircuitBreakerOpenError("open"),
        )
        checker.register_parser(parser)
        status = await checker.check_parser("test")
        assert status.is_healthy is False
        assert "Circuit breaker open" in status.last_error

    @pytest.mark.asyncio
    async def test_check_unregistered_parser(self):
        checker = ParserHealthChecker()
        status = await checker.check_parser("nonexistent")
        assert status.is_enabled is False
        assert status.is_healthy is False
        assert status.last_error == "Parser not registered"

    @pytest.mark.asyncio
    async def test_check_empty_events(self):
        checker = ParserHealthChecker()
        parser = MockParser(slug="test", name="Test", events=[])
        checker.register_parser(parser)
        status = await checker.check_parser("test")
        assert status.is_healthy is True


class TestParserHealthCheckerDisable:
    @pytest.mark.asyncio
    async def test_auto_disable_after_threshold(self):
        checker = ParserHealthChecker(failure_threshold=3)
        parser = MockParser(
            slug="test",
            name="Test",
            raise_on_call=RuntimeError("fail"),
        )
        checker.register_parser(parser)
        for _ in range(3):
            await checker.check_parser("test")
        status = checker.statuses["test"]
        assert status.is_enabled is False
        assert status.disabled_at is not None
        assert "consecutive failures" in status.disabled_reason

    @pytest.mark.asyncio
    async def test_disabled_parser_skips_check(self):
        checker = ParserHealthChecker(failure_threshold=1, cooldown_seconds=9999)
        parser = MockParser(
            slug="test",
            name="Test",
            raise_on_call=RuntimeError("fail"),
        )
        checker.register_parser(parser)
        await checker.check_parser("test")
        assert checker.statuses["test"].is_enabled is False
        prev_checks = checker.statuses["test"].total_checks
        status = await checker.check_parser("test")
        assert status.total_checks == prev_checks


class TestParserHealthCheckerReEnable:
    @pytest.mark.asyncio
    async def test_re_enable_after_cooldown(self):
        checker = ParserHealthChecker(failure_threshold=1, cooldown_seconds=0.01)
        parser = MockParser(
            slug="test",
            name="Test",
            raise_on_call=RuntimeError("fail"),
        )
        checker.register_parser(parser)
        await checker.check_parser("test")
        assert checker.statuses["test"].is_enabled is False
        await asyncio.sleep(0.02)
        parser._raise_on_call = None
        status = await checker.check_parser("test")
        assert status.is_enabled is True
        assert status.consecutive_failures == 0


class TestParserHealthCheckerManual:
    def test_enable_parser(self):
        checker = ParserHealthChecker()
        parser = MockParser(slug="test", name="Test")
        checker.register_parser(parser)
        checker.disable_parser("test", "manual")
        assert checker.statuses["test"].is_enabled is False
        checker.enable_parser("test")
        assert checker.statuses["test"].is_enabled is True
        assert checker.statuses["test"].disabled_at is None
        assert checker.statuses["test"].consecutive_failures == 0

    def test_disable_parser(self):
        checker = ParserHealthChecker()
        parser = MockParser(slug="test", name="Test")
        checker.register_parser(parser)
        checker.disable_parser("test", "maintenance")
        assert checker.statuses["test"].is_enabled is False
        assert checker.statuses["test"].disabled_reason == "maintenance"

    def test_enable_nonexistent_parser(self):
        checker = ParserHealthChecker()
        checker.enable_parser("nonexistent")

    def test_disable_nonexistent_parser(self):
        checker = ParserHealthChecker()
        checker.disable_parser("nonexistent")


class TestParserHealthCheckerQueries:
    def test_get_enabled_parsers(self):
        checker = ParserHealthChecker()
        checker.register_parser(MockParser(slug="a", name="A"))
        checker.register_parser(MockParser(slug="b", name="B"))
        checker.disable_parser("b")
        enabled = checker.get_enabled_parsers()
        assert "a" in enabled
        assert "b" not in enabled

    def test_get_disabled_parsers(self):
        checker = ParserHealthChecker()
        checker.register_parser(MockParser(slug="a", name="A"))
        checker.register_parser(MockParser(slug="b", name="B"))
        checker.disable_parser("b")
        disabled = checker.get_disabled_parsers()
        assert "b" in disabled
        assert "a" not in disabled

    @pytest.mark.asyncio
    async def test_check_all(self):
        checker = ParserHealthChecker()
        checker.register_parser(MockParser(slug="a", name="A", events=[1]))
        checker.register_parser(MockParser(slug="b", name="B", events=[2]))
        results = await checker.check_all()
        assert "a" in results
        assert "b" in results
        assert results["a"].is_healthy is True
        assert results["b"].is_healthy is True

    def test_get_summary(self):
        checker = ParserHealthChecker()
        checker.register_parser(MockParser(slug="a", name="A", events=[1]))
        checker.register_parser(MockParser(slug="b", name="B", events=[1]))
        checker.disable_parser("b")
        summary = checker.get_summary()
        assert summary["total_parsers"] == 2
        assert summary["enabled"] == 1
        assert summary["disabled"] == 1
        assert "a" in summary["parsers"]
        assert "b" in summary["parsers"]
        assert summary["parsers"]["a"]["is_enabled"] is True
        assert summary["parsers"]["b"]["is_enabled"] is False
