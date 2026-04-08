# scanner/parsers/health.py
"""
Parser health checker with auto-disable and re-enable capabilities.
Monitors parser health, reports success rates, and manages broken parsers.
"""
import time
import asyncio
import logging
from typing import Dict, Optional, List, TYPE_CHECKING
from dataclasses import dataclass, field

from scanner.parsers.circuit_breaker import CircuitBreaker, CircuitBreakerOpenError

if TYPE_CHECKING:
    from scanner.parsers.base import BaseParser

logger = logging.getLogger(__name__)


@dataclass
class ParserHealthStatus:
    slug: str
    name: str
    is_enabled: bool = True
    is_healthy: bool = True
    success_rate: float = 100.0
    total_checks: int = 0
    failed_checks: int = 0
    last_check_time: Optional[float] = None
    last_success_time: Optional[float] = None
    last_error: Optional[str] = None
    disabled_at: Optional[float] = None
    disabled_reason: Optional[str] = None
    consecutive_failures: int = 0
    response_time_ms: float = 0


class ParserHealthChecker:
    """
    Monitors parser health and auto-manages broken parsers.

    Features:
    - Tests each parser periodically
    - Reports success rate and response time
    - Auto-disables parsers after too many failures
    - Auto-re-enables after cooldown period
    """

    def __init__(
        self,
        failure_threshold: int = 5,
        cooldown_seconds: float = 300.0,
        check_timeout: float = 15.0,
    ):
        self.failure_threshold = failure_threshold
        self.cooldown_seconds = cooldown_seconds
        self.check_timeout = check_timeout

        self.statuses: Dict[str, ParserHealthStatus] = {}
        self._parsers: Dict[str, "BaseParser"] = {}
        self._lock = asyncio.Lock()

    def register_parser(self, parser: "BaseParser"):
        """Register a parser for health monitoring."""
        self._parsers[parser.slug] = parser
        if parser.slug not in self.statuses:
            self.statuses[parser.slug] = ParserHealthStatus(
                slug=parser.slug,
                name=parser.name,
            )

    async def check_parser(self, slug: str) -> ParserHealthStatus:
        """Run a health check on a specific parser."""
        async with self._lock:
            status = self.statuses.get(slug)
            parser = self._parsers.get(slug)

            if not parser or not status:
                return ParserHealthStatus(
                    slug=slug,
                    name=slug,
                    is_enabled=False,
                    is_healthy=False,
                    last_error="Parser not registered",
                )

            if not status.is_enabled:
                if self._should_re_enable(status):
                    status.is_enabled = True
                    status.disabled_at = None
                    status.disabled_reason = None
                    status.consecutive_failures = 0
                    logger.info(f"Parser '{slug}' re-enabled after cooldown")
                else:
                    return status

        start_time = time.time()
        try:
            timeout_future = asyncio.wait_for(parser.get_events(), timeout=self.check_timeout)
            events = await timeout_future

            response_time = (time.time() - start_time) * 1000

            async with self._lock:
                status.total_checks += 1
                status.consecutive_failures = 0
                status.last_check_time = time.time()
                status.last_success_time = time.time()
                status.is_healthy = True
                status.response_time_ms = response_time

                if len(events) == 0:
                    logger.debug(f"Parser '{slug}' returned 0 events (may be normal)")

            logger.debug(f"Health check '{slug}': OK ({len(events)} events, {response_time:.0f}ms)")

        except asyncio.TimeoutError:
            async with self._lock:
                self._record_failure(status, f"Timeout after {self.check_timeout}s")
        except CircuitBreakerOpenError as e:
            async with self._lock:
                self._record_failure(status, f"Circuit breaker open: {e}")
        except Exception as e:
            async with self._lock:
                self._record_failure(status, str(e))

        return status

    def _record_failure(self, status: ParserHealthStatus, error: str):
        """Record a health check failure."""
        status.total_checks += 1
        status.failed_checks += 1
        status.consecutive_failures += 1
        status.last_check_time = time.time()
        status.last_error = error
        status.is_healthy = False

        if status.consecutive_failures >= self.failure_threshold:
            status.is_enabled = False
            status.disabled_at = time.time()
            status.disabled_reason = f"{status.consecutive_failures} consecutive failures"
            logger.warning(
                f"Parser '{status.slug}' DISABLED: {status.disabled_reason}"
            )

    def _should_re_enable(self, status: ParserHealthStatus) -> bool:
        """Check if a disabled parser should be re-enabled."""
        if not status.disabled_at:
            return True
        elapsed = time.time() - status.disabled_at
        return elapsed >= self.cooldown_seconds

    async def check_all(self) -> Dict[str, ParserHealthStatus]:
        """Run health checks on all registered parsers."""
        results = {}
        for slug in list(self._parsers.keys()):
            results[slug] = await self.check_parser(slug)
        return results

    def get_enabled_parsers(self) -> List[str]:
        """Get list of enabled parser slugs."""
        return [
            slug for slug, status in self.statuses.items()
            if status.is_enabled
        ]

    def get_disabled_parsers(self) -> List[str]:
        """Get list of disabled parser slugs."""
        return [
            slug for slug, status in self.statuses.items()
            if not status.is_enabled
        ]

    def enable_parser(self, slug: str):
        """Manually enable a parser."""
        if slug in self.statuses:
            self.statuses[slug].is_enabled = True
            self.statuses[slug].disabled_at = None
            self.statuses[slug].disabled_reason = None
            self.statuses[slug].consecutive_failures = 0
            logger.info(f"Parser '{slug}' manually enabled")

    def disable_parser(self, slug: str, reason: str = "Manual"):
        """Manually disable a parser."""
        if slug in self.statuses:
            self.statuses[slug].is_enabled = False
            self.statuses[slug].disabled_at = time.time()
            self.statuses[slug].disabled_reason = reason
            logger.info(f"Parser '{slug}' manually disabled: {reason}")

    def get_summary(self) -> dict:
        """Get health summary for all parsers."""
        total = len(self.statuses)
        enabled = len(self.get_enabled_parsers())
        disabled = len(self.get_disabled_parsers())
        healthy = sum(1 for s in self.statuses.values() if s.is_healthy)

        parser_details = {}
        for slug, status in self.statuses.items():
            parser_details[slug] = {
                "name": status.name,
                "is_enabled": status.is_enabled,
                "is_healthy": status.is_healthy,
                "success_rate": round(
                    (status.total_checks - status.failed_checks) / max(status.total_checks, 1) * 100, 2
                ),
                "total_checks": status.total_checks,
                "failed_checks": status.failed_checks,
                "consecutive_failures": status.consecutive_failures,
                "last_error": status.last_error,
                "response_time_ms": round(status.response_time_ms, 2),
                "disabled_reason": status.disabled_reason,
            }

        return {
            "total_parsers": total,
            "enabled": enabled,
            "disabled": disabled,
            "healthy": healthy,
            "unhealthy": total - healthy,
            "parsers": parser_details,
        }
