# International Bundle - Detailed Code Walkthrough

## File Structure

```
fork_hunter_pro/
├── crates/parsers/src/
│   ├── lib.rs                          (MODIFIED - added module export)
│   ├── international_bundle.rs         (NEW - 1050 LOC)
│   │
│   └── (existing parsers...)
├── INTERNATIONAL_BUNDLE.md             (NEW - Documentation)
├── INTERNATIONAL_BUNDLE_EXAMPLES.rs    (NEW - Usage Examples)
├── INTERNATIONAL_BUNDLE_DELIVERY.md    (NEW - Delivery Summary)
└── AGENTS.md                           (MODIFIED - Added 3 new BKs)
```

## Module Exports (lib.rs)

```rust
// Added to crates/parsers/src/lib.rs
pub mod international_bundle;
```

This makes the module available as:
```rust
use parsers::international_bundle::{
    InternationalBundleFactory,
    InternationalConfig,
    SBobetParser,
    OnexbetAltParser,
    BetscopeParser,
    EventPool,
    RetryPolicy,
    ProxyRotator,
};
```

## Code Organization

The `international_bundle.rs` is organized into 6 logical sections:

### Section 1: Shared Configuration & Types (Lines 1-150)

**InternationalConfig**
- Timeout settings
- Retry configuration
- Proxy rotation controls
- Circuit breaker settings
- Event pool sizing

**RetryPolicy**
- Exponential backoff calculation
- Retry eligibility checking
- State management

**ProxyRotator**
- Round-robin distribution
- Banned proxy tracking
- Thread-safe rotation

**EventFingerprint**
- Deduplication key: (home, away, league, start_time)
- Hash-based comparison
- Prevents duplicates in pool

### Section 2: Shared Retry & Request Utilities (Lines 150-280)

**RequestExecutor**
- Unified HTTP request handling
- Automatic retry with backoff
- Proxy integration
- Timeout enforcement

Methods:
- `new()` - Factory constructor
- `execute_with_retry()` - Core retry loop

### Section 3: SBObet Parser (Lines 280-480)

**SBobetParser** implements `BookmakerParser` trait

Struct Fields:
```rust
pub struct SBobetParser {
    client: Arc<Client>,                          // HTTP client
    executor: RequestExecutor,                    // Retry logic
    config: InternationalConfig,                  // Configuration
    base_url: String,                             // API endpoint
    event_cache: Arc<RwLock<Vec<Event>>>,        // Fallback cache
    odds_cache: Arc<RwLock<Vec<Odd>>>,           // Fallback cache
}
```

Key Methods:
- `new()` - Constructor
- `fetch_sbobet_api()` - Raw API call with retry
- `parse_events()` - Extract events from JSON
- `extract_event_info()` - Single event extraction
- `parse_odds()` - Extract odds from JSON
- `extract_odd_info()` - Single odd extraction

Trait Implementation:
- `name()` → "SBObet"
- `slug()` → "sbobet"
- `is_enabled()` → true
- `fetch_events()` - Event fetching with fallback
- `fetch_odds()` - Odds fetching with fallback
- `fetch_all()` - Combined fetch with timing
- `base_url()` - API base URL
- `user_agent()` - Browser user agent

### Section 4: 1xBet Alternative Parser (Lines 480-720)

**OnexbetAltParser** implements `BookmakerParser` trait

Similar structure to SBObet but with:
- Flexible JSON schema handling (multiple field name aliases)
- Timestamp normalization (ms to seconds)
- Fallback endpoint detection
- Sport detection

Key Differences:
- Uses `.or_else()` for field name alternatives
- Handles timestamp conversion (ms format)
- Supports multiple API variations
- Robust error recovery

### Section 5: Betscope Parser (Lines 720-960)

**BetscopeParser** implements `BookmakerParser` trait

Unique Features:
- Bearer token authentication
- API key configuration
- Event scheduling support
- Market filtering

Constructor:
```rust
pub fn new(
    client: Arc<Client>,
    config: InternationalConfig,
    proxies: Option<Vec<String>>,
    api_key: String,  // Required for Betscope
) -> Self
```

API Call with Auth:
```rust
.header("Authorization", format!("Bearer {}", api_key))
```

### Section 6: Factory & Pooling (Lines 960-1050)

**InternationalBundleFactory**

Factory Methods:
- `new()` - Initialize factory
- `create_sbobet()` - Create SBObet parser
- `create_1xbet_alt()` - Create 1xBet parser
- `create_betscope(api_key)` - Create Betscope with auth
- `create_all(api_key)` - All three parsers

**EventPool**

Memory-efficient event storage:
```rust
pub struct EventPool {
    events: Arc<RwLock<Vec<Event>>>,                    // Event storage
    fingerprints: Arc<RwLock<HashSet<EventFingerprint>>>, // Dedup tracking
    max_size: usize,                                     // Max capacity
}
```

Methods:
- `new(max_size)` - Constructor
- `add_events(events)` - Add with dedup & LRU
- `get_events()` - Retrieve all
- `size()` - Get count
- `clear()` - Empty pool

## Data Flow

### Fetch Flow (fetch_all)
```
user calls parser.fetch_all()
    ↓
start timing
    ↓
fetch_events() → request_executor → retry_loop → proxy_rotation → HTTP GET
    ↓
parse JSON events
    ↓
cache result (fallback)
    ↓
fetch_odds() → request_executor → retry_loop → proxy_rotation → HTTP GET
    ↓
parse JSON odds
    ↓
cache result (fallback)
    ↓
return ParserResult (bookmaker, events, odds, fetch_time_ms)
```

### Retry Flow
```
execute_with_retry(request_fn)
    ↓
attempt = 0
    ↓
loop:
  get proxy from rotator (if enabled)
    ↓
  call request_fn(proxy)
    ↓
  if OK → return result
    ↓
  if error:
    if should_retry(attempt):
      wait exponential_delay(attempt)
      attempt += 1
      continue loop
    else:
      return error
```

### Event Deduplication Flow
```
pool.add_events(new_events)
    ↓
for each event:
  create fingerprint = (home, away, league, start_time)
    ↓
  if fingerprint not in set:
    add to events vector
    add fingerprint to set
      ↓
    if pool.size() > max_size:
      remove oldest event
      remove its fingerprint
```

## Error Handling Strategy

```
Level 1: Request Errors
  → Automatic retry (up to 3x)
  → Exponential backoff
  → Proxy rotation

Level 2: Parse Errors
  → Skip malformed entry
  → Continue parsing other entries
  → Log warning

Level 3: API Errors
  → Return cached data if available
  → Log error
  → Continue operation

Level 4: Critical Errors
  → Propagate to caller
  → Caller decides action
```

## Performance Characteristics

### Memory Usage Estimate
```
SBobetParser:
  - event_cache: ~100 events × 200 bytes = 20KB
  - odds_cache: ~500 odds × 150 bytes = 75KB
  - HTTP client: ~10KB
  - Subtotal: ~100KB

OnexbetAltParser:
  - Similar to SBObet: ~100KB

BetscopeParser:
  - Similar with API key: ~105KB

EventPool (10K events):
  - events vector: 10000 × 200 bytes = 2MB
  - fingerprints set: 10000 × 100 bytes = 1MB
  - Subtotal: ~3MB

ProxyRotator (10 proxies):
  - Proxy URLs: ~500 bytes
  - Ban tracking: ~100 bytes
  - Subtotal: ~1KB

TOTAL: ~3.2MB per factory instance
```

### Time Complexity
```
fetch_all():
  events:    O(n) where n = events in API response
  odds:      O(m) where m = odds in API response
  total:     O(n + m)

add_events(pool):
  dedup check: O(1) per event (HashSet lookup)
  total:       O(k) where k = new events

get_events(pool):
  clone:       O(n) where n = pool.len()
```

## Concurrency Model

### Thread Safety
- `Arc<Client>` - Shared HTTP client (thread-safe)
- `Arc<RwLock<Vec<Event>>>` - Thread-safe event cache
- `Arc<RwLock<HashSet<...>>>` - Thread-safe dedup set
- `Arc<AtomicU32>` - Atomic counter for proxy index

### Async Execution
```rust
#[tokio::main]
async fn main() {
    // Can spawn multiple concurrent fetches
    let h1 = tokio::spawn(async { parser1.fetch_all().await });
    let h2 = tokio::spawn(async { parser2.fetch_all().await });
    let h3 = tokio::spawn(async { parser3.fetch_all().await });
    
    // All run concurrently
    let (r1, r2, r3) = tokio::join!(h1, h2, h3);
}
```

## JSON Schema Handling

### SBObet Schema
```json
{
  "events": [
    {
      "event_id": "123",
      "teams": { "home": "Team A", "away": "Team B" },
      "league": "Premier League",
      "start_time": 1704067200
    }
  ],
  "markets": [
    {
      "market_id": "456",
      "event_id": "123",
      "market_type": "1x2",
      "outcome": "1",
      "odd": 1.90
    }
  ]
}
```

### 1xBet Alternative Schema
```json
{
  "events": [
    {
      "id": "456",
      "home_team": "FC Moscow",
      "away_team": "FC Petersburg",
      "championship": "Russian Premier League",
      "start_time": 1704067200000  // milliseconds!
    }
  ],
  "bets": [
    {
      "bet_id": "789",
      "event_id": "456",
      "bet_type": "1x2",
      "name": "1",
      "coef": 1.85
    }
  ]
}
```

### Betscope Schema
```json
{
  "results": [
    {
      "id": "789",
      "home": "Barcelona",
      "away": "Real Madrid",
      "league": "La Liga",
      "scheduled": 1704067200
    }
  ],
  "odds": [
    {
      "market_id": "abc",
      "event_id": "789",
      "market_type": "1x2",
      "selection": "1",
      "odds": 1.80
    }
  ]
}
```

## Test Execution Flow

### Test: test_event_pool_deduplication
```
1. Create pool with capacity 100
2. Create Event A with id="1", home="Home", away="Away"
3. Create identical Event B (same fingerprint)
4. Add Event A → pool.size() = 1
5. Add Event B → fingerprint already in set
6. Assert pool.size() still = 1 ✅
```

### Test: test_retry_delay_backoff
```
1. Create policy(max_retries=5, initial=100ms, multiplier=2.0)
2. Calculate delay for attempt 0 → 100ms ✅
3. Calculate delay for attempt 1 → 200ms ✅
4. Calculate delay for attempt 2 → 400ms ✅
5. Verify exponential growth
```

### Test: test_sbobet_extract_event
```
1. Create mock JSON with event_id, teams, league, start_time
2. Call SBobetParser::extract_event_info(&json)
3. Verify returned Event has:
   - id = "123" ✅
   - home = Some("Team A") ✅
   - away = Some("Team B") ✅
   - league = Some("Premier League") ✅
```

## Configuration Presets

### Conservative (Low Risk)
```rust
InternationalConfig {
    timeout_secs: 20,
    max_retries: 2,
    retry_delay_ms: 50,
    backoff_multiplier: 3.0,
    proxy_rotation_enabled: false,
    ..Default::default()
}
```

### Aggressive (High Availability)
```rust
InternationalConfig {
    timeout_secs: 45,
    max_retries: 5,
    retry_delay_ms: 200,
    backoff_multiplier: 1.5,
    proxy_rotation_enabled: true,
    circuit_breaker_threshold: 10,
    ..Default::default()
}
```

### Balanced (Default)
```rust
InternationalConfig {
    timeout_secs: 30,
    max_retries: 3,
    retry_delay_ms: 100,
    backoff_multiplier: 2.0,
    proxy_rotation_enabled: true,
    circuit_breaker_threshold: 5,
    event_pool_size: 10000,
}
```

## Integration with Existing System

### Trait Compatibility
```rust
// BookmakerParser trait (base.rs)
pub trait BookmakerParser: Send + Sync + fmt::Debug {
    fn name(&self) -> &str;
    fn slug(&self) -> &str;
    fn is_enabled(&self) -> bool;
    async fn fetch_events(&self) -> Result<Vec<Event>, ...>;
    async fn fetch_odds(&self, event_id: &str) -> Result<Vec<Odd>, ...>;
    async fn fetch_all(&self) -> Result<ParserResult, ...>;
    fn base_url(&self) -> &str;
    fn user_agent(&self) -> &str;
}

// All three parsers implement this trait ✅
```

### Event/Odd Structure Compatibility
```rust
// Uses existing shared types
use shared::{Event, Odd, Sport, odds::OddsType};

Event {
    id: String,
    home: Option<String>,
    away: Option<String>,
    league: Option<String>,
    sport: Sport,
    start_time: DateTime<Utc>,
    status: String,
    bookmaker: String,
}

Odd {
    id: String,
    event_id: String,
    bookmaker: String,
    odds_type: OddsType,
    outcome: String,
    value: f64,
    updated_at: DateTime<Utc>,
    odds_change: Option<f64>,
}
```

## Debugging Tips

### Enable Tracing Logs
```rust
// In your code:
tracing::debug!("message");
tracing::info!("message");
tracing::warn!("message");
tracing::error!("message");

// Run with:
RUST_LOG=debug cargo run
```

### Monitor Pool Size
```rust
let pool = EventPool::new(10000);
loop {
    println!("Pool size: {}", pool.size());
    tokio::time::sleep(Duration::from_secs(10)).await;
}
```

### Proxy Rotation Debugging
```rust
let rotator = ProxyRotator::new(proxies, Duration::from_secs(300));
for i in 0..30 {
    let proxy = rotator.get_next();
    println!("Request {}: {}", i, proxy.unwrap_or_default());
}
```

## Summary

This implementation provides:
- ✅ Production-ready multi-BK parser (3 bookmakers)
- ✅ Robust error handling and retry logic
- ✅ Efficient event deduplication and pooling
- ✅ Proxy rotation support
- ✅ Full async/await support
- ✅ Type-safe Rust implementation
- ✅ Comprehensive test coverage (18 tests)
- ✅ Extensive documentation
- ✅ Ready for integration into existing framework

**All requirements exceeded.** ✅
