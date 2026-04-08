# scanner/parsers/circuit_breaker.py
"""
Circuit breaker pattern implementation for parser resilience.
Prevents cascading failures when a bookmaker API is down.
"""
import time
import asyncio
import logging
from enum import Enum
from typing import Callable, Any, Optional
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


class CircuitState(Enum):
    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half_open"


@dataclass
class CircuitBreakerStats:
    total_calls: int = 0
    successful_calls: int = 0
    failed_calls: int = 0
    last_failure_time: Optional[float] = None
    last_success_time: Optional[float] = None
    last_state_change: Optional[float] = None
    consecutive_failures: int = 0
    consecutive_successes: int = 0


class CircuitBreakerOpenError(Exception):
    """Raised when circuit breaker is open and call is rejected."""
    pass


class CircuitBreaker:
    """
    Circuit breaker for parser resilience.

    States:
    - CLOSED: Normal operation, calls pass through
    - OPEN: Too many failures, calls are rejected immediately
    - HALF_OPEN: Testing if service recovered, one call allowed
    """

    def __init__(
        self,
        slug: str,
        failure_threshold: int = 5,
        recovery_timeout: float = 60.0,
        half_open_max_calls: int = 1,
        success_threshold: int = 2,
    ):
        self.slug = slug
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.half_open_max_calls = half_open_max_calls
        self.success_threshold = success_threshold

        self.state = CircuitState.CLOSED
        self.stats = CircuitBreakerStats()
        self._half_open_calls = 0
        self._lock = asyncio.Lock()

    async def call(self, func: Callable, *args, **kwargs) -> Any:
        async with self._lock:
            if self.state == CircuitState.OPEN:
                if self._should_attempt_reset():
                    self._transition_to(CircuitState.HALF_OPEN)
                else:
                    raise CircuitBreakerOpenError(
                        f"Circuit breaker '{self.slug}' is OPEN. "
                        f"Recovering in {self._time_until_reset():.0f}s"
                    )

            if self.state == CircuitState.HALF_OPEN:
                if self._half_open_calls >= self.half_open_max_calls:
                    raise CircuitBreakerOpenError(
                        f"Circuit breaker '{self.slug}' is HALF_OPEN, max test calls reached"
                    )
                self._half_open_calls += 1

        try:
            result = await func(*args, **kwargs) if asyncio.iscoroutinefunction(func) else func(*args, **kwargs)
            await self._on_success()
            return result
        except Exception as e:
            await self._on_failure(e)
            raise

    async def _on_success(self):
        async with self._lock:
            self.stats.total_calls += 1
            self.stats.successful_calls += 1
            self.stats.consecutive_successes += 1
            self.stats.consecutive_failures = 0
            self.stats.last_success_time = time.time()

            if self.state == CircuitState.HALF_OPEN:
                if self.stats.consecutive_successes >= self.success_threshold:
                    self._transition_to(CircuitState.CLOSED)

    async def _on_failure(self, error: Exception):
        async with self._lock:
            self.stats.total_calls += 1
            self.stats.failed_calls += 1
            self.stats.consecutive_failures += 1
            self.stats.consecutive_successes = 0
            self.stats.last_failure_time = time.time()

            if self.state == CircuitState.HALF_OPEN:
                self._transition_to(CircuitState.OPEN)
            elif self.stats.consecutive_failures >= self.failure_threshold:
                self._transition_to(CircuitState.OPEN)
                logger.warning(
                    f"Circuit breaker '{self.slug}' OPENED after "
                    f"{self.stats.consecutive_failures} consecutive failures"
                )

    def _should_attempt_reset(self) -> bool:
        if self.stats.last_failure_time is None:
            return True
        elapsed = time.time() - self.stats.last_failure_time
        return elapsed >= self.recovery_timeout

    def _time_until_reset(self) -> float:
        if self.stats.last_failure_time is None:
            return 0
        elapsed = time.time() - self.stats.last_failure_time
        return max(0, self.recovery_timeout - elapsed)

    def _transition_to(self, new_state: CircuitState):
        old_state = self.state
        self.state = new_state
        self.stats.last_state_change = time.time()
        if new_state == CircuitState.CLOSED:
            self.stats.consecutive_failures = 0
            self._half_open_calls = 0
        elif new_state == CircuitState.HALF_OPEN:
            self._half_open_calls = 0
        logger.info(
            f"Circuit breaker '{self.slug}': {old_state.value} -> {new_state.value}"
        )

    def reset(self):
        """Manually reset circuit breaker to CLOSED state."""
        self._transition_to(CircuitState.CLOSED)
        self.stats.consecutive_failures = 0
        self.stats.consecutive_successes = 0

    @property
    def success_rate(self) -> float:
        if self.stats.total_calls == 0:
            return 100.0
        return round(self.stats.successful_calls / self.stats.total_calls * 100, 2)

    def get_status(self) -> dict:
        return {
            "slug": self.slug,
            "state": self.state.value,
            "success_rate": self.success_rate,
            "total_calls": self.stats.total_calls,
            "successful_calls": self.stats.successful_calls,
            "failed_calls": self.stats.failed_calls,
            "consecutive_failures": self.stats.consecutive_failures,
            "consecutive_successes": self.stats.consecutive_successes,
            "last_failure_time": self.stats.last_failure_time,
            "last_success_time": self.stats.last_success_time,
            "recovery_timeout": self.recovery_timeout,
            "time_until_reset": self._time_until_reset() if self.state == CircuitState.OPEN else 0,
        }
