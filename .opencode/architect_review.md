# 🏗️ ARCHITECT REVIEW — Ghost Imperium

**Date:** 2026-03-31
**Reviewer:** ARCHITECT (Lead Developer & Designer)
**Scope:** Full codebase review for production readiness
**Status:** 🔴 CRITICAL ISSUES FOUND

---

## 1. Architecture Issues Found

### 1.1 Global State & Dependency Injection (CRITICAL)
**Location:** `api/main.py:24`, `api/routes.py:8`, `api/websocket.py:79`

```python
# api/main.py
scanner: Optional[GhostScanner] = None

# api/routes.py
scanner = None
def set_scanner(s):
    global scanner
    scanner = s
```

**Problems:**
- Global mutable state makes testing impossible without side effects
- Circular import pattern: `websocket.py` imports `scanner` from `api.routes` at runtime
- No dependency injection — scanner is injected via side-effect function call
- Race conditions possible if multiple requests access `scanner` simultaneously
- Cannot run multiple scanner instances (e.g., for different configurations)

**Impact:** High — blocks horizontal scaling, makes testing fragile, introduces race conditions.

---

### 1.2 Tight Coupling Between Layers (HIGH)
**Location:** `scanner/engine.py:208-209`

```python
from services.analytics import analytics_engine
await analytics_engine.record_surebet(sb)
```

**Problems:**
- Scanner directly imports and calls analytics singleton — violates single responsibility
- Scanner knows about database internals (`self.database.save_surebet(sb)`)
- No event bus or message queue between components
- Adding new notification channels (email, webhook) requires modifying engine

**Impact:** High — every new feature requires modifying core engine.

---

### 1.3 Missing Parser Factory (HIGH)
**Location:** `scanner/engine.py:86-114`, `scanner/parsers/__init__.py`

**Problems:**
- Parser initialization logic is hardcoded in engine `_init_parsers()`
- Playwright fallback logic is duplicated and fragile
- No way to register parsers dynamically
- `ALL_PARSERS` list must be manually maintained
- Mock parser switching is done via env var check inside engine (violates SRP)

**Impact:** High — adding/removing parsers requires code changes in multiple places.

---

### 1.4 Database Schema Issues (HIGH)
**Location:** `services/database.py:16-44`

**Problems:**
- **No indexes** on any columns — full table scans on every query
- `surebets` table has no index on `found_at`, `sport`, `profit_percent`
- `stakes` table has no foreign key constraint to `surebets`
- No `created_at`/`updated_at` timestamps for auditing
- `data TEXT` column stores entire JSON — defeats purpose of relational DB
- No migration system — schema changes will break in production
- `INSERT OR REPLACE` on surebets silently overwrites history
- No connection pooling configuration
- No WAL mode enabled for SQLite (concurrent reads blocked)

**Impact:** High — performance degrades rapidly as data grows.

---

### 1.5 Memory Leaks & Unbounded Collections (MEDIUM)
**Location:** `services/analytics.py:17-18`, `scanner/engine.py:71-72`

```python
# analytics.py
self.surebet_history: List[Dict] = []  # Grows until 10000, then truncates
self.profit_history: List[Dict] = []   # Never cleaned up

# engine.py
self._cycle_times: List[float] = []  # Bounded to 100
self.seen_surebet_ids: set = set()   # NEVER cleaned — grows forever
```

**Problems:**
- `seen_surebet_ids` grows indefinitely — memory leak
- `profit_history` is declared but never used
- No TTL on in-memory collections
- No backpressure mechanism when memory is full

**Impact:** Medium — will cause OOM crashes after days of continuous operation.

---

### 1.6 Performance Bottlenecks (MEDIUM)
**Location:** `core/normalizer.py:114-144`, `core/finder.py:126-136`

**Problems:**
- **Levenshtein distance** calculated on EVERY event comparison — O(n×m) per comparison
- Team normalizer runs fuzzy matching against ALL known teams for EVERY event
- No caching of normalization results (cache declared but unused in hot path)
- Event grouping in finder creates new dict every cycle
- `_detect_incremental_changes` hashes string concatenation — fragile and slow

**Impact:** Medium — normalization becomes the bottleneck as event count grows.

---

### 1.7 Inconsistent Error Handling (MEDIUM)
**Location:** Throughout codebase

**Problems:**
- Bare `except:` clauses everywhere (`scanner/engine.py:133`, `scanner/engine.py:168`)
- No structured error types — all errors are generic exceptions
- Parser failures silently return empty lists — no alerting
- No retry with exponential backoff for transient failures
- No circuit breaker pattern for failing bookmakers

**Impact:** Medium — failures are silent, hard to debug, no recovery strategy.

---

### 1.8 API Design Issues (MEDIUM)
**Location:** `api/routes.py`

**Problems:**
- Inconsistent response formats:
  - `/surebets` returns `ApiResponse` wrapper
  - `/surebets/top` returns raw dict `{"surebets": [...]}`
  - `/stats` returns raw dict
  - `/health` returns raw dict
- No pagination metadata (no `next_cursor`, `total_pages`)
- No rate limiting on API endpoints
- No authentication/authorization
- Calculator endpoint uses query params for complex data (`odds=1.5,2.5`)
- No input validation on most endpoints
- Bonuses endpoint has hardcoded data — should be in config/DB

**Impact:** Medium — inconsistent API is hard to consume and maintain.

---

### 1.9 Logging Strategy (LOW)
**Location:** Throughout codebase

**Problems:**
- Mixed logging: `structlog` in `api/main.py`, `logging` everywhere else
- No log levels strategy (INFO vs DEBUG vs WARNING)
- No correlation IDs for request tracing
- No structured logging in most modules
- No log rotation or retention policy
- Emojis in log messages break log parsers

**Impact:** Low — makes production debugging difficult.

---

### 1.10 Configuration Management (LOW)
**Location:** `scanner/engine.py:34-39`

```python
config = ScannerConfig(
    min_profit=0.5,
    cycle_interval=3.0,
    max_events_per_source=200,
    cache_ttl=10.0
)
```

**Problems:**
- Hardcoded config in `api/main.py` — should come from env/settings
- No config validation on startup
- No way to change config at runtime
- No config per-bookmaker (rate limits, timeouts)

**Impact:** Low — reduces flexibility for different deployment environments.

---

## 2. Refactoring Plan

### Priority 1: CRITICAL (Do Immediately)

| # | Change | Why | Risk | Priority |
|---|--------|-----|------|----------|
| 1.1 | Implement dependency injection container | Eliminates global state, enables testing, allows multiple instances | Medium | 🔴 P0 |
| 1.2 | Add database indexes | Prevents full table scans, critical for performance | Low | 🔴 P0 |
| 1.3 | Fix `seen_surebet_ids` memory leak | Prevents OOM crashes | Low | 🔴 P0 |
| 1.4 | Standardize API response format | Consistent client experience | Low | 🔴 P0 |

### Priority 2: HIGH (Do This Sprint)

| # | Change | Why | Risk | Priority |
|---|--------|-----|------|----------|
| 2.1 | Create ParserFactory | Decouples parser registration from engine | Low | 🟠 P1 |
| 2.2 | Add event bus / pub-sub for notifications | Decouples scanner from analytics/telegram | Medium | 🟠 P1 |
| 2.3 | Add database migration system (Alembic) | Safe schema evolution | Low | 🟠 P1 |
| 2.4 | Add API rate limiting | Prevents abuse | Low | 🟠 P1 |
| 2.5 | Add circuit breaker for parsers | Graceful degradation when BK is down | Medium | 🟠 P1 |

### Priority 3: MEDIUM (Do Next Sprint)

| # | Change | Why | Risk | Priority |
|---|--------|-----|------|----------|
| 3.1 | Optimize team normalizer with caching | Reduces CPU usage | Low | 🟡 P2 |
| 3.2 | Add structured logging everywhere | Better observability | Low | 🟡 P2 |
| 3.3 | Add input validation (Pydantic) on all endpoints | Prevents bad data | Low | 🟡 P2 |
| 3.4 | Add correlation IDs for request tracing | Easier debugging | Low | 🟡 P2 |
| 3.5 | Move hardcoded bonuses to config/DB | Maintainability | Low | 🟡 P2 |

### Priority 4: LOW (Backlog)

| # | Change | Why | Risk | Priority |
|---|--------|-----|------|----------|
| 4.1 | Add authentication/authorization | Security for production | Medium | 🟢 P3 |
| 4.2 | Add config validation on startup | Fail fast on bad config | Low | 🟢 P3 |
| 4.3 | Add health check endpoints | K8s/load balancer support | Low | 🟢 P3 |
| 4.4 | Add metrics export (Prometheus) | Production monitoring | Medium | 🟢 P3 |

---

## 3. Design Patterns to Add

### 3.1 Factory Pattern — Parser Registry
**Why:** Decouple parser creation from engine, enable dynamic registration.

```python
class ParserRegistry:
    _parsers: Dict[str, Type[BaseParser]] = {}
    
    @classmethod
    def register(cls, parser_cls: Type[BaseParser]):
        cls._parsers[parser_cls.slug] = parser_cls
    
    @classmethod
    def create(cls, slug: str, **kwargs) -> BaseParser:
        return cls._parsers[slug](**kwargs)
    
    @classmethod
    def create_all(cls, slugs: Set[str], **kwargs) -> List[BaseParser]:
        return [cls.create(s, **kwargs) for s in slugs if s in cls._parsers]
```

**Usage:**
```python
@parser_registry.register
class WinlineParser(BaseParser):
    slug = "winline"
```

---

### 3.2 Observer Pattern — Event Bus
**Why:** Decouple scanner from notification consumers (Telegram, WebSocket, analytics).

```python
class EventBus:
    _subscribers: Dict[str, List[Callable]] = defaultdict(list)
    
    def subscribe(self, event_type: str, callback: Callable):
        self._subscribers[event_type].append(callback)
    
    async def publish(self, event_type: str, data: Any):
        for callback in self._subscribers.get(event_type, []):
            try:
                await callback(data)
            except Exception as e:
                logger.error(f"Subscriber error: {e}")

# Events: "surebet.found", "cycle.completed", "parser.error"
```

---

### 3.3 Strategy Pattern — Market Detection
**Why:** Different market types (1x2, totals, handicaps) need different detection logic.

```python
class MarketStrategy(ABC):
    @abstractmethod
    def detect(self, events: List[Event]) -> List[Surebet]: ...

class TwoWayStrategy(MarketStrategy): ...
class ThreeWayStrategy(MarketStrategy): ...
class TotalStrategy(MarketStrategy): ...
class HandicapStrategy(MarketStrategy): ...

class SurebetCalculator:
    def __init__(self, strategies: List[MarketStrategy]):
        self.strategies = strategies
    
    def find_surebets(self, events: List[Event]) -> List[Surebet]:
        return [
            sb for strategy in self.strategies
            for sb in strategy.detect(events)
        ]
```

---

### 3.4 Repository Pattern — Data Access
**Why:** Abstract database operations, enable swapping SQLite → PostgreSQL.

```python
class SurebetRepository(ABC):
    @abstractmethod
    async def save(self, surebet: Surebet) -> None: ...
    
    @abstractmethod
    async def get_recent(self, limit: int) -> List[Surebet]: ...
    
    @abstractmethod
    async def get_by_sport(self, sport: str, limit: int) -> List[Surebet]: ...

class SQLiteSurebetRepository(SurebetRepository): ...
class PostgresSurebetRepository(SurebetRepository): ...
```

---

### 3.5 Circuit Breaker Pattern — Parser Resilience
**Why:** Prevent cascading failures when a bookmaker API is down.

```python
class CircuitBreaker:
    def __init__(self, failure_threshold: int = 5, recovery_timeout: float = 60):
        self.failure_count = 0
        self.state = "closed"  # closed, open, half-open
    
    async def call(self, func: Callable, *args, **kwargs):
        if self.state == "open":
            raise CircuitOpenError()
        try:
            result = await func(*args, **kwargs)
            self.failure_count = 0
            return result
        except Exception:
            self.failure_count += 1
            if self.failure_count >= self.failure_threshold:
                self.state = "open"
            raise
```

---

### 3.6 Unit of Work Pattern — Database Transactions
**Why:** Ensure atomic operations across multiple tables.

```python
class UnitOfWork:
    async def __aenter__(self):
        self.db = await aiosqlite.connect(self.path)
        await self.db.execute("BEGIN")
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if exc_type:
            await self.db.rollback()
        else:
            await self.db.commit()
        await self.db.close()
```

---

## 4. API Design Review

### Current State: ⚠️ Needs Improvement

| Endpoint | Format | Pagination | Validation | Auth | Rating |
|----------|--------|------------|------------|------|--------|
| `GET /api/v1/surebets` | ApiResponse ✅ | ❌ | Partial ❌ | ❌ | 5/10 |
| `GET /api/v1/surebets/top` | Raw dict ❌ | ❌ | ❌ | ❌ | 3/10 |
| `GET /api/v1/events` | ApiResponse ✅ | ❌ | Partial ❌ | ❌ | 5/10 |
| `GET /api/v1/stats` | Raw dict ❌ | N/A | ❌ | ❌ | 4/10 |
| `GET /api/v1/bookmakers` | Raw dict ❌ | N/A | ❌ | ❌ | 4/10 |
| `GET /api/v1/bonuses` | Raw dict ❌ | N/A | ❌ | ❌ | 3/10 |
| `POST /api/v1/scanner/start` | Raw dict ❌ | N/A | ❌ | ❌ | 3/10 |
| `GET /api/v1/calculator` | ApiResponse ✅ | N/A | Partial ❌ | ❌ | 6/10 |
| `GET /api/v1/search` | Raw dict ❌ | ❌ | ❌ | ❌ | 3/10 |
| `GET /health` | Raw dict ❌ | N/A | ❌ | ❌ | 5/10 |

### Recommendations:

1. **Standardize all responses** to use `ApiResponse` wrapper
2. **Add pagination** with cursor-based pagination for large datasets
3. **Add request/response validation** using Pydantic models
4. **Add rate limiting** per IP/user
5. **Add API key authentication** for production
6. **Add OpenAPI tags** for better documentation grouping
7. **Add response caching** headers for static endpoints
8. **Add versioning strategy** (URL versioning is good, keep it)

### Suggested Endpoint Improvements:

```python
# Add pagination metadata
class PaginatedResponse(BaseModel):
    items: List[Any]
    total: int
    page: int
    page_size: int
    has_next: bool

# Add proper error responses
class ApiError(BaseModel):
    code: str
    message: str
    details: Optional[Dict] = None

# Add filtering with Pydantic
class SurebetFilter(BaseModel):
    min_profit: float = 0.5
    sport: Optional[str] = None
    live_only: bool = False
    bookmakers: Optional[List[str]] = None
```

---

## 5. Database Schema Review

### Current State: 🔴 Needs Major Improvements

### Missing Indexes:
```sql
-- CRITICAL: Add these indexes
CREATE INDEX IF NOT EXISTS idx_surebets_found_at ON surebets(found_at);
CREATE INDEX IF NOT EXISTS idx_surebets_sport ON surebets(sport);
CREATE INDEX IF NOT EXISTS idx_surebets_profit ON surebets(profit_percent);
CREATE INDEX IF NOT EXISTS idx_surebets_live ON surebets(is_live);
CREATE INDEX IF NOT EXISTS idx_stakes_surebet_id ON stakes(surebet_id);
CREATE INDEX IF NOT EXISTS idx_stakes_status ON stakes(status);
```

### Schema Issues:

| Issue | Severity | Fix |
|-------|----------|-----|
| No foreign keys | HIGH | Add `FOREIGN KEY (surebet_id) REFERENCES surebets(id)` |
| No indexes | HIGH | Add indexes listed above |
| No WAL mode | MEDIUM | `PRAGMA journal_mode=WAL;` |
| No `created_at` | MEDIUM | Add timestamp columns |
| JSON in TEXT column | MEDIUM | Extract key fields to columns |
| No migrations | HIGH | Use Alembic |
| No connection pooling | MEDIUM | Use `aiosqlite` pool or switch to PostgreSQL |
| `INSERT OR REPLACE` loses history | HIGH | Use `INSERT` with unique constraint on `id` |

### Recommended Schema:

```sql
CREATE TABLE surebets (
    id TEXT PRIMARY KEY,
    event_name TEXT NOT NULL,
    sport TEXT NOT NULL,
    profit_percent REAL NOT NULL,
    total_stake REAL NOT NULL,
    estimated_profit REAL NOT NULL,
    bookmakers TEXT NOT NULL,
    market_type TEXT NOT NULL,
    is_live INTEGER NOT NULL DEFAULT 0,
    data TEXT,
    found_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_surebets_found_at ON surebets(found_at DESC);
CREATE INDEX idx_surebets_sport ON surebets(sport);
CREATE INDEX idx_surebets_profit ON surebets(profit_percent DESC);
CREATE INDEX idx_surebets_live ON surebets(is_live);

CREATE TABLE stakes (
    id TEXT PRIMARY KEY,
    surebet_id TEXT NOT NULL,
    bookmaker TEXT NOT NULL,
    event_name TEXT NOT NULL,
    selection TEXT NOT NULL,
    odds REAL NOT NULL,
    stake_amount REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    placed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (surebet_id) REFERENCES surebets(id) ON DELETE CASCADE
);

CREATE INDEX idx_stakes_surebet_id ON stakes(surebet_id);
CREATE INDEX idx_stakes_status ON stakes(status);
```

---

## 6. Code Quality Score

### Module Ratings (1-10)

| Module | Score | Justification |
|--------|-------|---------------|
| **api/main.py** | 5/10 | Good FastAPI setup, but global state, mixed concerns, redundant event handlers |
| **api/routes.py** | 4/10 | Inconsistent responses, no validation, no pagination, global scanner |
| **api/websocket.py** | 6/10 | Clean WS manager, but circular import risk, no reconnection logic |
| **scanner/engine.py** | 5/10 | Core logic is solid, but tightly coupled, memory leaks, hardcoded config |
| **scanner/parsers/base.py** | 7/10 | Good abstraction, retry logic, rate limiting. Could use circuit breaker |
| **core/finder.py** | 6/10 | Correct math, but duplicated logic for 2-way/3-way, no strategy pattern |
| **core/cache.py** | 8/10 | Well-implemented TTL cache, rate limiter. Good use of threading/asyncio locks |
| **core/normalizer.py** | 5/10 | Good concept, but Levenshtein is O(n²), no caching, fuzzy matching is slow |
| **services/database.py** | 3/10 | Bare minimum, no indexes, no migrations, no connection pooling, no FK |
| **services/analytics.py** | 5/10 | Functional but memory-leaky, no persistence, no aggregation optimization |
| **models/surebet.py** | 8/10 | Clean Pydantic models, good use of enums, proper typing |
| **models/event.py** | 8/10 | Well-structured, good use of enums, proper typing |
| **core/normalizer.py** | 5/10 | Good concept but performance issues with fuzzy matching |

### Overall Codebase Score: **5.5/10**

**Strengths:**
- Good use of async/await
- Pydantic models are well-defined
- Parser abstraction is solid
- Cache implementation is good
- Core surebet math is correct

**Weaknesses:**
- Global state everywhere
- No dependency injection
- Database schema is production-unready
- Memory leaks in long-running processes
- Inconsistent API responses
- No testing infrastructure visible
- No error handling strategy
- Performance bottlenecks in normalizer

---

## 7. Production Readiness Checklist

| Requirement | Status | Notes |
|-------------|--------|-------|
| Dependency injection | ❌ | Global state everywhere |
| Database migrations | ❌ | No Alembic |
| Database indexes | ❌ | No indexes on any table |
| Memory leak fixes | ❌ | `seen_surebet_ids` grows forever |
| Error handling strategy | ❌ | Bare `except:` everywhere |
| Rate limiting (API) | ❌ | No rate limits on endpoints |
| Authentication | ❌ | No auth at all |
| Structured logging | ⚠️ | Mixed logging libraries |
| Health checks | ⚠️ | Basic `/health` endpoint |
| Monitoring/metrics | ❌ | No Prometheus/Grafana |
| CI/CD pipeline | ❌ | No GitHub Actions |
| Docker configuration | ❓ | Dockerfile exists but not reviewed |
| Testing | ❓ | Tests directory exists but coverage unknown |
| Documentation | ⚠️ | ARCHITECTURE.md exists, no API docs |

---

## 8. Recommended Next Steps

1. **Week 1:** Fix critical issues (DI, memory leaks, DB indexes)
2. **Week 2:** Implement design patterns (Factory, Observer, Strategy)
3. **Week 3:** Add production features (rate limiting, auth, monitoring)
4. **Week 4:** Testing, documentation, CI/CD

---

*Review completed by ARCHITECT. All findings are actionable and prioritized.*
