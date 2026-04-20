# Olimp Parser - Proxy & Resilience Implementation

## 📋 SUMMARY

**Status**: ✅ READY FOR MERGE

Разблокирован Olimp парсер (HTTP 403 IP ban) с:
- ✅ Автоматической ротацией прокси
- ✅ Circuit breaker паттерном
- ✅ Exponential backoff retry стратегией
- ✅ Health checks для прокси листа
- ✅ Полное логирование

## 🏗️ ARCHITECTURE

### Files Created/Modified

| File | Status | Changes |
|------|--------|---------|
| `crates/parsers/src/proxy_manager.rs` | ✅ NEW | ProxyManager: ротация, health checks, banned tracking |
| `crates/parsers/src/olimp.rs` | ✅ UPDATED | Proxy integration + circuit breaker + retry logic |
| `crates/parsers/src/lib.rs` | ✅ UPDATED | Added `pub mod proxy_manager` |

### Components

#### 1. ProxyManager (`proxy_manager.rs`)
```rust
pub struct ProxyManager {
    proxies: Arc<RwLock<Vec<ProxyState>>>,
    current_index: Arc<AtomicU32>,
}
```

**Features**:
- Ротация прокси с взвешиванием по success rate
- Трекинг failed/success count
- Automatic banning на 10+ минут при 403
- Health checks по fail rate (>0.6 → unhealthy)

**API**:
```rust
let manager = ProxyManager::new(vec![
    ProxyConfig::http("proxy1:8080"),
    ProxyConfig::socks5("proxy2:1080"),
]);

// Get next healthy proxy (weighted random)
if let Some(proxy) = manager.get_next_proxy() { ... }

// Mark results
manager.mark_success(&proxy_url);
manager.mark_failure(&proxy_url);
manager.mark_banned(&proxy_url, Duration::from_secs(600));

// Check health
let status = manager.health_status(); // Vec<(url, is_healthy, success_rate)>
let count = manager.healthy_count();
```

#### 2. OlimpParser with Resilience (`olimp.rs`)

**Retry Strategy** (Exponential Backoff):
- Max 3 attempts
- Initial backoff: 100ms
- Max backoff: 5000ms
- Multiplier: 2.0x

**Logic Flow**:
```
1. Check circuit breaker (allow request?)
   └─ Fail: Return "Circuit breaker open"

2. Try direct request (no proxy)
   ├─ Success: Parse & return
   ├─ HTTP 403: Fall through to proxy
   └─ Other error: Return error

3. Try with proxy (if available)
   ├─ Success: Mark proxy as healthy, return
   ├─ HTTP 403: Ban proxy for 10 min, retry
   └─ Other: Mark failure, retry next proxy

4. If all proxies exhausted: Return error

5. On retry: Exponential backoff before next attempt

6. Circuit breaker:
   ├─ Closed: Allow all requests
   ├─ Open: Block requests (timeout = 60s)
   └─ HalfOpen: Test with 2 successes to close
```

**Constructor**:
```rust
// Without proxies (direct requests only)
let parser = OlimpParser::new(client);

// With proxies (for IP ban bypass)
let proxies = vec![
    ProxyConfig::http("107.1.1.1:8080"),
    ProxyConfig::socks5("192.168.1.100:1080"),
];
let parser = OlimpParser::with_proxies(client, proxies);
```

## 📊 USAGE EXAMPLES

### Example 1: Direct Usage (No Proxy)
```rust
use std::sync::Arc;
use parsers::OlimpParser;

let client = Arc::new(reqwest::Client::new());
let parser = OlimpParser::new(client);

// Will try direct request first
let events = parser.fetch_events().await?;
```

### Example 2: With Proxy Rotation
```rust
use parsers::{OlimpParser, proxy_manager::ProxyConfig};

let proxies = vec![
    ProxyConfig::http("proxy1.example.com:8080"),
    ProxyConfig::socks5("proxy2.example.com:1080"),
];

let parser = OlimpParser::with_proxies(client, proxies);

// If direct request gets 403:
// 1. Will try proxy1
// 2. If proxy1 also banned: try proxy2
// 3. If all banned: return error
// 4. On failure: exponential backoff + retry

let events = parser.fetch_events().await?;
```

### Example 3: Health Monitoring
```rust
// Check proxy health status
if let Some(status) = parser.proxy_health_status() {
    for (url, is_healthy, success_rate) in status {
        println!("{}: healthy={}, rate={:.2}", url, is_healthy, success_rate);
    }
}

// Check healthy count
println!("Healthy proxies: {}", parser.healthy_proxy_count());
```

## 🧪 TESTS

### Test Coverage (5 tests added)

1. **`creates_parser_with_proxies`** - Verify parser initialization with proxy list
2. **`circuit_breaker_starts_closed`** - Verify circuit breaker initial state
3. **`readiness_snapshot_includes_proxy_rotation`** - Verify diagnostic check for proxy support
4. **`status_code_extraction`** - Verify HTTP error code parsing (403, 429, etc)
5. **`proxy_manager_tests`** (in proxy_manager.rs) - 6 tests for proxy rotation:
   - Proxy health tracking
   - Ban/unban functionality
   - Success rate calculation
   - Proxy selection

### Running Tests
```bash
cargo test --lib parsers
cargo test --lib parsers::proxy_manager
cargo test --lib parsers::olimp
```

## 🔧 CONFIGURATION

### Default Settings
```rust
// Retry settings
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 5000;
const BACKOFF_MULTIPLIER: f64 = 2.0;

// Circuit breaker
CircuitBreaker::new(
    3,    // failure_threshold (open after 3 failures)
    60,   // recovery_timeout_secs (try recovery after 60s)
    2,    // half_open_max (need 2 successes to close)
)

// Proxy ban duration
Duration::from_secs(600)  // 10 minutes
```

### Customization
To customize, edit olimp.rs:
```rust
// Change max retries
const MAX_RETRIES: u32 = 5;

// Change circuit breaker threshold
CircuitBreaker::new(5, 120, 3)

// Change ban duration
Duration::from_secs(1800)  // 30 minutes
```

## 📝 LOG OUTPUT

### Example Logs
```
INFO Olimp: initializing with proxies proxy_count=3
DEBUG Olimp: fetching section url=...
WARN Olimp: IP banned (403), attempting proxy rotation
DEBUG Olimp: attempting with proxy proxy=107.1.1.1:8080
INFO Olimp: request successful via proxy proxy=107.1.1.1:8080
INFO Olimp: recovered after 2 attempts attempts=2
INFO Olimp events parsed count=445
DEBUG Olimp: parsed sports=14 events=445 odds=2340
```

### Error Cases
```
WARN Olimp: proxy IP also banned (403) proxy=107.1.1.1:8080
WARN Olimp: no healthy proxies available
ERROR Olimp: fetch failed after 3 attempts section=live
```

## ✅ VERIFICATION CHECKLIST

- [x] ProxyManager with rotation & health checks
- [x] Circuit breaker integration
- [x] Exponential backoff retry strategy
- [x] HTTP 403 detection & handling
- [x] Proxy ban tracking (10 min cooldown)
- [x] Fallback to direct request if proxies unavailable
- [x] Full async/await support
- [x] Thread-safe with Arc<RwLock>
- [x] Comprehensive logging (debug/info/warn/error)
- [x] 5 unit tests (all passing)
- [x] No breaking changes to existing API
- [x] Backward compatible (new() still works without proxies)

## 🚀 DEPLOYMENT

### Step 1: Verify Compilation
```bash
cargo check --lib parsers
```

### Step 2: Run Tests
```bash
cargo test --lib parsers
```

### Step 3: Build Release
```bash
cargo build --release
```

### Step 4: Update ParserFactory (if needed)
In `parser_factory.rs`, update initialization:
```rust
// Old: without proxies
let parser = Arc::new(OlimpParser::new(client.clone()));

// New: with proxies from config
let proxies = vec![
    ProxyConfig::http("proxy1:8080"),
    // Add more from environment or config...
];
let parser = Arc::new(OlimpParser::with_proxies(client.clone(), proxies));
```

## 📦 Dependencies

All dependencies already in Cargo.toml:
- `tokio` - async runtime
- `reqwest` - HTTP client with proxy support
- `parking_lot` - RwLock for thread-safe state
- `rand` - weighted proxy selection
- `tracing` - logging

## ⚠️ KNOWN LIMITATIONS

1. **Proxy timeout**: Fixed at 30 seconds per request
   - Modify: `Duration::from_secs(30)` in `execute_request`

2. **Proxy list static**: Must pass at initialization
   - Future: Could add dynamic proxy list updates

3. **Ban duration fixed**: Always 10 minutes
   - Modify: `Duration::from_secs(600)` in `mark_banned`

## 🔄 FUTURE IMPROVEMENTS

1. Dynamic proxy list loading from API/file
2. Proxy geolocation diversity checking
3. Adaptive backoff based on response time
4. Prometheus metrics for proxy health
5. Machine learning for proxy selection
6. Rate limiting per proxy
7. Geographic rotation for IP diversity

---

**Author**: Fork Hunter Pro Proxy Team  
**Date**: 2026-04-18  
**Version**: 0.1.0  
**Status**: ✅ READY TO MERGE
