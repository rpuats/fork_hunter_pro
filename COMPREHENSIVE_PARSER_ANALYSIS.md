# Comprehensive Fork Hunter Pro Parser Analysis
**Date**: April 2026 | **Status**: Production Analysis | **Scope**: All 35+ Parsers

---

## Executive Summary

**Total Parsers**: 35 (23 Rust Native + 12 Partially Implemented)
- **Production Working**: 7/7 ✅ (Pari, Fonbet, Marathon, Bettery, Bet24, Leon, Sportbet)
- **Diagnostics/Rollout Ready**: 8 (Winline, Betboom, Betcity, Baltbet, Zenit, BetBoom, Olimp, OlimpBet)
- **Blocked by Anti-Scraping**: 1xBet, Winline (JavaScript required)
- **Not Yet Ported**: Liga Stavok, Tennis, мБет (schema mismatches)
- **International Bundle**: 3 (SBObet, 1xBet Alt, Betscope) ✅

**Key Finding**: Market efficiently protects entry points but public APIs remain accessible through:
- Direct API endpoints (Pari, Marathon, Fonbet, Bettery)
- HTTP + proxy rotation (Olimp, Baltbet, Zenit)
- Rendered page extraction + circuit breaker (Winline, Betboom, BetBoom)

---

## Part 1: CRATES STRUCTURE

### Core Crates Organization

```
crates/
├── parsers/                 # Main parser implementations
│   └── src/
│       ├── base.rs         # BookmakerParser trait
│       ├── parser_factory.rs  # Factory pattern, registry
│       ├── circuit_breaker.rs  # Fallback management
│       ├── proxy_manager.rs    # Proxy rotation & health
│       ├── stealth.rs          # UA rotation, headers
│       ├── headless_helper.rs  # Chrome headless utilities
│       │
│       ├── [WORKING PARSERS]
│       ├── pari.rs         ✅ API (scope=2300)
│       ├── fonbet.rs       ✅ API (scope=1600)
│       ├── marathon.rs     ✅ API (scope=3000)
│       ├── bettery.rs      ✅ API (scope=1500)
│       ├── bet24.rs        ✅ API
│       ├── leon.rs         ✅ API
│       ├── sportbet.rs     ✅ API
│       ├── international_bundle.rs  ✅ SBObet, 1xBet Alt, Betscope
│       │
│       ├── [ROLLOUT READY - DIAGNOSTICS MODE]
│       ├── winline.rs      ⚠️ Headless-first SPA parser
│       ├── betboom.rs      ⚠️ Rendered page extraction
│       ├── betcity.rs      ⚠️ HTTP API + HTML fallback
│       ├── baltbet.rs      ⚠️ Pure HTTP + proxy
│       ├── zenit.rs        ⚠️ HTTP + imprinthash auth
│       ├── olimpbet.rs     ⚠️ API (no Cloudflare)
│       ├── olimp.rs        ⚠️ HTTP + proxy rotation
│       │
│       ├── [PARTIAL/LEGACY]
│       ├── melbet.rs       ⏳ Headless (legacy Playwright)
│       ├── betm.rs         ⏳ Headless (legacy)
│       ├── tennisi.rs      ⏳ HTTP parser (needs fixes)
│       ├── tennis.rs       ⏳ ATP/WTA parser (schema mismatch)
│       ├── mbet.rs         ⏳ мБет parser (schema mismatch)
│       ├── liga_stavok.rs  ⏳ Liga parser (QRATOR blocker)
│       ├── ligastavok.rs   ⏳ Duplicate Liga variant
│       │
│       └── [DISABLED/BLOCKED]
│       ├── 1xbet.rs        ❌ Geoblocked (403 access denied)
│       ├── 1x_stavka.rs    ❌ Geoblocked
│       ├── olimp_old.rs    ❌ Deprecated
│
├── shared/                  # Shared types
│   └── src/models.rs        # Event, Odd, Sport, ParserReadiness
│
├── engine/                  # Calculation engine
├── auto_betting/            # Betting automation (StealthBetting)
├── monitoring/              # Health checks
└── ...
```

---

## Part 2: PARSER STATUS MATRIX

### ✅ PRODUCTION WORKING (7/7)

| Parser | Type | API Path | Events | Auth | Status | Notes |
|--------|------|----------|--------|------|--------|-------|
| **Pari** | API | `line-lb01-w.pb06e2-resources.com` | ~6600 | None | ✅ Live | Direct JSON, no proxy needed |
| **Fonbet** | API | `line-lb61-w.bk6bba-resources.com` | ~6800 | None | ✅ Live | Shared platform (scope=1600) |
| **Marathon** | API | `line51.tf39be-resources.com` | ~6500 | None | ✅ Live | Shared platform (scope=3000) |
| **Bettery** | API | Shared platform | ~6800 | None | ✅ Live | Factor catalog support |
| **Bet24 (24bet)** | API | Direct API | ~6500 | None | ✅ Live | Fixed Sport::Other bug (Apr 2026) |
| **Leon** | API | `leon.ru/api-2/betline` | ~3600 | None | ✅ Live | Inplay + prematch |
| **Sportbet** | API | Direct API | ~250 | None | ✅ Live | Small but reliable |

#### Key Characteristics:
- **Single HTTP GET** to dedicated API endpoints
- **No JavaScript rendering** required
- **No proxy rotation** typically needed (open public APIs)
- **Response format**: Nested JSON with events & factors
- **Timeout**: 30 seconds
- **Retry**: Exponential backoff (500ms → 5s)

**Sample Pari Request:**
```bash
GET https://line-lb01-w.pb06e2-resources.com/events/list?lang=ru&scopeMarket=2300
Headers: User-Agent, Accept-Language, Accept-Encoding: gzip
Response: { events: [...], customFactors: [...] }
```

---

### ⚠️ ROLLOUT READY / DIAGNOSTICS MODE (8)

These parsers **have working implementations** but are **not in production** due to either:
- Transient nightly regressions (zero events)
- Structural issues under investigation
- Needing strict runtime validation before promotion

#### Category A: Headless/Rendered Page Extractors

**Winline** (headless-first SPA)
- **Status**: ⚠️ Diagnostics mode
- **Tech**: Chrome headless + JavaScript evaluation
- **Blocking Method**: JavaScript-rendered SPA (no pre-rendered HTML)
- **Anti-Detection**: Desktop profile, viewport (1440x2200), dynamic UA rotation
- **Current Issue**: Found 0 events via JS extraction (bridge test)
- **Timeout**: 70 seconds total execution budget
- **Routes**: 18 prematch pages, 8 live pages, 2 scroll rounds
- **Code**: [winline.rs](crates/parsers/src/winline.rs) - **160+ lines of headless logic**

**BetBoom** (rendered page extraction)
- **Status**: ⚠️ Diagnostics mode  
- **Tech**: Chrome headless + Sporthub proto discovery
- **Blocking Method**: JS-rendered, protobuf data transport (Sporthub WebSocket)
- **Anti-Detection**: Profile rotation, focused probe URLs for football
- **Bootstrap Detection**: Looks for `sporthub-feed.proto`, protobuf markers
- **Future**: Protobuf frame decoder (runtime-gated, not yet implemented)
- **Timeout**: 45 seconds wall-clock limit
- **Code**: [betboom.rs](crates/parsers/src/betboom.rs) - Full Sporthub integration scaffolded

**Melbet** (legacy headless)
- **Status**: ⏳ Partially implemented
- **Tech**: Headless Chrome (legacy Playwright port)
- **Issue**: From old Python bridge, needs validation in Rust
- **Code**: [melbet.rs](crates/parsers/src/melbet.rs)

#### Category B: HTTP APIs with Fallbacks & Proxy

**Olimp** (public competitions-with-events API)
- **Status**: ⚠️ Re-enabled, rollout ready
- **Tech**: HTTP JSON API + proxy rotation + circuit breaker
- **Blocking Method**: HTTP 403 IP bans
- **Proxy Features**:
  - Geolocation-aware rotation (RU, US, DE, NL, UA, BY, KZ)
  - Health check intervals: Healthy (10s), Degraded (3s), Banned (300s)
  - Success rate tracking
  - Exponential backoff retry: 3 attempts, 100ms-5s backoff
- **Endpoints**: `/api/v4/0/live` and `/api/v4/0/line/top`
- **Last Probe** (2026-04-18): 445 live + 1110 prematch events ✅
- **Production Block**: Awaiting strict Rust-side volume validation
- **Code**: [olimp.rs](crates/parsers/src/olimp.rs) - Full proxy integration

**Zenit** (pure HTTP + header auth)
- **Status**: ⚠️ Rollout ready, nightly regressed
- **Tech**: HTTP API with `imprinthash` + `frontversion` headers
- **Blocking Method**: Requires authentication headers captured from browser
- **Headers**: 
  ```
  imprinthash: d01d68e5a9775b90a0c7239e7f078895 (default)
  frontversion: 1.72.1 (default)
  ```
- **Endpoints**: `/ajax/line/printer/react` (prematch), `/ajax/live/printer/react` (live)
- **Response**: `{ games: {...}, dict: {...} }`
- **Recent Runtime**: 182 live + 3497 prematch (meets nightly KPI)
- **Current Issue**: Recent strict nightly regressed to 0 events (transient?)
- **Code**: [zenit.rs](crates/parsers/src/zenit.rs)

**Betcity** (HTTP API + HTML fallback)
- **Status**: ⚠️ Rollout ready
- **Tech**: Direct API (`ad.betcity.ru`) + HTML scraping fallback
- **Blocking Method**: Occasional transient 429/502/503
- **Retry Logic**: 3 attempts, 500ms-5s exponential backoff
- **Last Direct Probe** (2026-04-18): 408 live + 6055 prematch ✅
- **Production Block**: Recent zero-event nightly looks transient (not structural)
- **Code**: [betcity.rs](crates/parsers/src/betcity.rs)

**Baltbet** (pure HTTP + legacy groups fallback)
- **Status**: ⚠️ Rollout ready
- **Tech**: JSON live endpoint + legacy HTML prematch discovery
- **Blocking Method**: Occasional rate limiting
- **Code**: [baltbet.rs](crates/parsers/src/baltbet.rs)

**OlimpBet** (API without Cloudflare)
- **Status**: ⚠️ Rollout ready
- **Tech**: Direct API (no Cloudflare protection)
- **Code**: [olimpbet.rs](crates/parsers/src/olimpbet.rs)

---

### ⏳ PARTIALLY IMPLEMENTED (6)

**Liga Stavok**
- **Status**: Partially coded but disabled
- **Blocker**: QRATOR bot protection (anti-bot challenge page)
- **Issue**: Bootstrap detection still experimental
- **Files**: [ligastavok.rs](crates/parsers/src/ligastavok.rs), [liga_stavok.rs](crates/parsers/src/liga_stavok.rs)

**Tennis** (ATP/WTA tournaments)
- **Status**: Partially coded, needs schema fixes
- **Implementation**: 3000+ events target for Grand Slams, Masters, 500/250
- **Markets**: Match winner, set betting, game betting, correct score
- **Issue**: Event/Odd struct schema mismatch
- **File**: [tennis.rs](crates/parsers/src/tennis.rs)

**мБет** (м-bet.com)
- **Status**: Partially coded, needs schema fixes
- **Implementation**: API parser with HTML fallback, proxy rotation
- **Target**: 4000+ events
- **Markets**: 1X2, Total, Corners, Cards
- **Issue**: Event/Odd struct schema mismatch
- **File**: [mbet.rs](crates/parsers/src/mbet.rs)

**Tennisi**
- **Status**: Partial HTTP implementation
- **Features**: Direct line/live HTML responses + category discovery
- **File**: [tennisi.rs](crates/parsers/src/tennisi.rs)

**Bet-M (betm)**
- **Status**: Headless parser (legacy Playwright port)
- **Notes**: Probes legacy and current public routes, needs validation
- **File**: [betm.rs](crates/parsers/src/betm.rs)

---

### ❌ BLOCKED (2)

**1xBet**
- **Status**: ❌ Geoblocked access denied
- **Error**: HTML response: "Доступ запрещен. Убедитесь, что ваша страна не находится в списке запрещённых на нашем сайте"
- **Blocking Method**: IP-based geolocation block (403 Forbidden)
- **Reason**: Non-Russian IPs denied
- **Proxy Testing Result**: Failed even with German proxy (DE/156.146.33.100)
- **Technical Issue**: VPN/proxy detection system actively blocks non-RU residence
- **File**: Not actively maintained in Rust (legacy Python only)

**1x_Stavka**
- **Status**: ❌ Geoblocked variant of 1xBet
- **Same Blocker**: Geographic IP filtering
- **File**: Deprecated

---

## Part 3: BLOCKING MECHANISMS ANALYSIS

### Category 1: Geographic IP Blocking

**Affected Bookmakers**:
- 1xBet (active enforcement)
- 1x_Stavka (same system)

**Technical Details**:
```
Response: 403 Forbidden
Body: "Доступ запрещен. Убедитесь, что ваша страна не находится в списке запрещённых"
IP Detection: GeoIP database lookup (MaxMind or similar)
Proxy Detection: VPN/proxy detection active (blocks known VPN IPs)
```

**Current Defense Mechanism**:
- Geolocation verification (latitude/longitude match)
- VPN/proxy signature database (frequently updated)
- Browser fingerprint verification (device fingerprint consistency)
- Metadata validation (timezone, language, system info)

**Evasion Gaps**:
- ❌ Basic proxy rotation doesn't help (still non-RU IP)
- ❌ User-Agent spoofing doesn't help (metadata-level check)
- ⚠️ Residential proxies (RU-based) would work but expensive

---

### Category 2: JavaScript Rendering Gate

**Affected Bookmakers**:
- Winline (SPA-first)
- BetBoom (Sporthub feed)
- Melbet (legacy headless)
- Liga Stavok (QRATOR + JS)

**Technical Details**:

**Winline**:
```
HTML Bootstrap: Empty <div id="root"></div>
SPA Rendering: React/Vue application loads in client
Data Load: WebSocket connection to wss://wss.winline.ru/data_ng
Commands: ["lang", "ru", "data", "WINLINE", "getdate"]
Event Filters: Events.filter({isLive:1}), Events.filter({isLive:0,...})
Line Data: Fetched via SM.PREDLINE or PREDLINELIVE commands
```

**BetBoom**:
```
Bootstrap: HTML mentions "sporthub" and protobuf
WebSocket: wss://sporthub.betboom.ru/ws
Protocol: Protobuf (length-delimited frames)
Channels: prematch_snapshot, live_update
Frame Structure: Binary length-delimited + payload
```

**Current Defense**:
- Headless Chrome execution (detectable by anti-bot scripts)
- No pre-rendered HTML (requires full SPA evaluation)
- WebSocket upgrade (additional fingerprinting surface)
- Frame parsing complexity (protobuf decoding needed)

---

### Category 3: Rate Limiting & Circuit Breaking

**Affected Bookmakers**:
- Betcity (occasional 429)
- Baltbet (rate limiting)
- Olimp (before proxy rotation)
- Marathon/Pari/Fonbet (rare)

**Technical Details**:
```
HTTP 429 Too Many Requests
Headers: Retry-After, X-RateLimit-Remaining
Detection: Connection pool exhaustion, rapid sequential requests
Solution: Exponential backoff (already implemented)
```

**Current Defense**:
- Circuit breaker pattern (state machine: Closed → Open → HalfOpen)
- Exponential backoff: 500ms → 1s → 2s → 5s (capped)
- Health tracking: Success rate % and response time
- Adaptive intervals: Healthy (10s check), Degraded (3s check)

---

### Category 4: QRATOR Bot Protection

**Affected Bookmakers**:
- Liga Stavok (active QRATOR protection)

**Technical Details**:
```
Bot Challenge: JavaScript puzzle or fingerprint challenge
Detection Method: Behavioral analysis (mouse movement, typing patterns, etc.)
Response: Challenge page on suspicious requests
Evasion: Requires solving QRATOR challenge or real browser interaction
```

**Current Status**:
- Bootstrap detection attempted
- No active solver (would need 2captcha integration or selenium interaction)

---

### Category 5: Proxy-Based Detection

**Detection Methods Used**:
- Known VPN/proxy database blacklisting
- BGP route analysis (datacenter IPs detected)
- TLS certificate validation
- HTTP header inconsistencies (datacenter signatures)
- Behavioral biometrics (click patterns, scroll timing)

**Affected By**:
- Olimp (HTTP 403 → proxy rotation fixes it)
- Winline (headless detection possible but not confirmed blocking)

**Current Defense**:
```rust
// From proxy_manager.rs
pub enum ProxyHealth {
    Healthy,      // Success rate >= 90%
    Degraded,     // Success rate 60-90%
    Unhealthy,    // Success rate < 60%
    Banned,       // Too many failures
}

pub struct ProxyManager {
    health_check_interval: Duration,  // Adaptive: 10s → 3s → 500ms
    success_rate_tracking: f64,
    response_time_averaging: [u64; 100],  // Rolling window
}
```

---

## Part 4: WORKING TECHNIQUES & PATTERNS

### Pattern A: Direct Public APIs (No Anti-Scraping)

**Parsers**: Pari, Fonbet, Marathon, Bettery, Leon, Sportbet, Bet24

**Why They Work**:
1. **No JavaScript Gate**: Full JSON response in initial HTML
2. **No Geographic Blocking**: Open international access
3. **No Rate Limiting**: Generous limits or per-IP quotas
4. **No Fingerprinting**: Standard HTTP request sufficient

**Implementation Pattern**:
```rust
// From pari.rs
async fn fetch_api(&self, url: &str, is_live: bool) -> Result<(Vec<Event>, Vec<Odd>)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .gzip(true)
        .build()?;
    
    let resp = client.get(url)
        .header("User-Agent", "Mozilla/5.0...")
        .header("Accept", "application/json")
        .header("Accept-Language", "ru-RU,ru;q=0.9")
        .header("Accept-Encoding", "gzip, deflate, br")
        .send()
        .await?;
    
    let json: serde_json::Value = resp.json().await?;
    Self::parse_api_response(&json, is_live)
}
```

**Key Success Factors**:
- ✅ Standard Mozilla User-Agent
- ✅ gzip/brotli compression support
- ✅ Reasonable timeout (30s)
- ✅ Language header for localization
- ✅ Accept-Encoding for bandwidth
- ✅ No exotic headers (avoids detection)

**Tested Against**: Pari, Fonbet, Marathon, Bettery
**Success Rate**: 100%

---

### Pattern B: HTTP + Proxy Rotation (For IP Blocking)

**Parsers**: Olimp, Baltbet, Zenit (when headers not available)

**Why They Work**:
1. **API Still Public**: No JS rendering needed
2. **IP-Based Blocking**: Can be rotated with proxies
3. **Stateless Requests**: Each request independent

**Implementation Pattern**:
```rust
// From olimp.rs
pub struct ProxyManager {
    pub url: String,
    pub country: Country,  // RU, US, DE, NL, UA, BY, KZ
    pub protocol: ProxyProtocol,  // HTTP, HTTPS, SOCKS5
}

async fn fetch_section_with_proxy(&self, section: &str) -> Result<(Vec<Event>, Vec<Odd>)> {
    let proxy = self.proxy_manager
        .get_next()
        .ok_or("No proxies available")?;
    
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&format!("http://{}", proxy))?)
        .timeout(Duration::from_secs(30))
        .build()?;
    
    let resp = client.get(&url)
        .header("User-Agent", self.random_ua())
        .send()
        .await?;
    
    self.circuit_breaker.record_success();
    Ok(parse_response(&resp)?)
}
```

**Proxy Rotation Features**:
- **Geolocation Awareness**: Rotate by country
- **Health Tracking**: Success rate per proxy
- **Adaptive Intervals**: Check healthy every 10s, degraded every 3s
- **Ban Tracking**: Temporarily ban failed proxies
- **Exponential Backoff**: 100ms → 500ms → 5s cap

**Tested Against**: Olimp
**Success Rate**: With proper proxies: ~95%

---

### Pattern C: Headless Chrome + Rendered Page Extraction

**Parsers**: Winline, BetBoom, Melbet

**Why They Work**:
1. **SPA Required**: React/Vue renders HTML client-side
2. **Full Browser Env**: Anti-bot scripts can verify browser legitimacy
3. **WebSocket Support**: Real-time data feeds

**Implementation Pattern**:
```rust
// From headless_helper.rs
pub struct HeadlessChromeHelper {
    browser: Browser,  // headless-chrome crate
}

impl HeadlessChromeHelper {
    pub fn navigate_and_wait(&self, url: &str, wait_ms: u64) -> Result<Tab> {
        let tab = self.browser.new_tab()?;
        
        // Set realistic profile
        tab.set_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64)...",
            Some("ru-RU,ru;q=0.9"),
            Some("Win32")
        )?;
        
        // Emulate viewport
        tab.set_bounds(Bounds::Normal {
            left: None,
            top: None,
            width: Some(1440),
            height: Some(2200),
        })?;
        
        // Navigate and wait for DOM
        tab.navigate_to(url)?;
        Self::wait_for_any_selector(&tab, &[
            "ww-feature-event",
            ".event-card",
            "[data-test*='event']",
        ], wait_ms)?;
        
        Ok(tab)
    }
}
```

**Anti-Detection Measures**:
- ✅ Desktop profile (not mobile)
- ✅ Realistic viewport (1440x2200)
- ✅ Platform spoofing (Win32)
- ✅ Language headers (ru-RU)
- ✅ Bootstrap JS execution
- ✅ DOM readiness waiting (not just page load)

**Current Issues**:
- ⚠️ Headless Chrome detection possible (chrome://version reveals headless flag)
- ⚠️ No mouse/keyboard simulation (WebDriver exposed)
- ⚠️ Window object lacks some browser properties

---

### Pattern D: Circuit Breaker for Fallback Management

**Used By**: Olimp, Zenit, Betcity, all major parsers

**Why It Works**:
1. **Graceful Degradation**: Open circuit prevents cascading failures
2. **Automatic Recovery**: Half-open state tests viability
3. **Metrics Tracking**: Health data for decisions

**Implementation**:
```rust
// From circuit_breaker.rs
pub enum CircuitState {
    Closed,      // Normal operation
    Open,        // Too many failures, reject requests
    HalfOpen,    // Testing recovery
}

pub fn record_failure(&self) {
    if *self.state == Closed && failures >= threshold {
        *self.state = Open;
        *self.last_failure_time = Some(Instant::now());
    }
}

pub fn allow_request(&self) -> bool {
    match *self.state {
        Closed => true,
        Open => {
            if last_failure.elapsed() >= recovery_timeout {
                *self.state = HalfOpen;
                true
            } else {
                false  // Circuit still open
            }
        }
        HalfOpen => true,  // Try recovering
    }
}
```

**Configuration**:
- Failure threshold: 2-3 failures
- Recovery timeout: 60 seconds
- Half-open max: 2 successful requests to close

---

## Part 5: DETAILED ANALYSIS OF BLOCKED PARSERS

### WINLINE - The JavaScript Wall

**Current Status**: 0 events extracted (bridge test showed: "Found 0 events via JS")

**Why It's Hard**:
```
1. SPA Architecture
   - No HTML bootstrap data
   - React/Vue renders events client-side
   
2. WebSocket Feed  
   - wss://wss.winline.ru/data_ng
   - Binary protocol (not JSON)
   - Commands: ["lang", "ru", "data", "WINLINE", "getdate"]
   - Filters: Events.filter({isLive:1})
   
3. Anti-Bot Countermeasures
   - WebDriver detection (chrome.webdriver)
   - Headless flag detection (navigator.webdriver)
   - DevTools protocol usage detection
```

**Current Implementation** (70+ seconds timeout):
```rust
const HEADLESS_RUNTIME_BUDGET_MS: u64 = 70_000;  // 70 seconds total
const HEADLESS_NAVIGATION_TIMEOUT_MS: u64 = 6_000;
const HEADLESS_PREMATCH_FANOUT_BUDGET_MS: u64 = 18_000;
const HEADLESS_LIVE_FANOUT_BUDGET_MS: u64 = 18_000;

// Routes tested:
const HEADLESS_MAX_PREMATCH_PAGES: usize = 18;  // Up to 18 sport pages
const HEADLESS_MAX_LIVE_SPORT_PAGES: usize = 8;   // Up to 8 live pages
const HEADLESS_SCROLL_ROUNDS: usize = 2;

// Extracted JS:
const HEADLESS_EXTRACT_JS: &str = r#"(() => {
    const normalizeText = (value) => (value || '').replace(/\s+/g, ' ').trim();
    // Extract event cards from rendered DOM
    // Return: [{ home, away, sport, odds: [{type, value}] }]
})"#;
```

**Extracted Technique Stack**:
1. **Navigation + Wait**: 6s timeout for navigation readiness
2. **DOM Polling**: Wait for event card selectors
3. **JavaScript Extraction**: Run extraction JS in page context
4. **Scroll Simulation**: 2 rounds of scrolling to load more events
5. **Multi-Route Fanout**: Test multiple sport pages in parallel

**Why It Still Returns 0**:
- ❌ Selectors mismatched against actual DOM structure
- ❌ WebSocket events not captured (only DOM snapshot)
- ❌ Anti-bot detection blocking headless execution
- ❌ Page load incomplete within timeout

**Potential Fixes**:
1. **Selector Analysis**: Inspect actual rendered HTML, update selectors
2. **WebSocket Interception**: Proxy WS frames instead of DOM extraction
3. **Anti-Bot Bypass**: 
   - Use puppeteer-extra-stealth plugin
   - Patch navigator.webdriver
   - Hide DevTools Protocol
4. **Longer Timeout**: Increase to 120s for full SPA initialization

---

### BETBOOM - Protobuf Feed Mystery

**Current Status**: ⚠️ Bootstrapped but not decoding frames

**What's Known**:
```javascript
// Bootstrap hints detected:
- sporthub-feed.proto
- sporthub namespace in HTML
- WebSocket URL: wss://sporthub.betboom.ru/ws
- Channels: prematch_snapshot, live_update
- Transport: Protobuf (length-delimited)
```

**Frame Structure**:
```
[Length byte 1-4] [Protobuf message payload]
E.g.: FF 01 02 03 [96 bytes of binary data]
```

**Current Gap**:
```rust
// From betboom.rs
pub(crate) fn runtime_enabled() -> bool {
    false  // Protobuf decoding still gated
}

// Scaffolded but not implemented:
pub(crate) enum FrameClass {
    Empty,
    JsonControl,
    TextHeartbeat,
    LengthDelimitedProtobuf,  // This one needed!
    BinaryOpaque,
}
```

**Implementation Status**:
- ✅ Bootstrap detection works
- ✅ Frame envelope classification exists
- ✅ ASCII/hex preview extraction ready
- ❌ Protobuf message definition missing
- ❌ Frame decoder not implemented
- ❌ WebSocket framing not tested in production

**To Unlock BetBoom**:
1. Capture real protobuf schema (from binary analysis or JS decompilation)
2. Generate Rust protobuf bindings (`protoc`)
3. Implement frame length-delimited parsing
4. Test WebSocket subscription flow
5. Map protobuf fields to Event/Odd structs

---

### 1xBET - Geolocation IP Blocking

**Current Status**: ❌ 403 Forbidden (Geoblocked)

**Blocking Mechanism**:
```
Request: GET https://1xbet.ru/
IP Detected: German datacenter (DE/156.146.33.100)
Response: 403 Forbidden
Body: "Доступ запрещен. Убедитесь, что ваша страна 
       не находится в списке запрещённых..."
```

**Technical Analysis**:
1. **GeoIP Database Lookup**: MaxMind or similar
   - Latitude/Longitude verification
   - ASN reputation check
   - ISP/datacenter fingerprinting
   
2. **VPN/Proxy Detection**:
   - Known datacenter IP ranges
   - BGP route analysis
   - Reverse DNS anomalies
   
3. **Browser Fingerprint Verification** (if proxy bypassed):
   - Timezone must match IP geolocation
   - Language preferences must match region
   - System locale must match country
   - Device type must be real (not VM)

**Why Proxy Rotation Failed**:
- Still an IP address (even if RU-based, recognized as VPN)
- VPN signature database blacklists known proxy providers
- Behavior biometrics (too many requests per IP in short time)

**Required for Success**:
1. **Residential Proxies** (actual home ISPs, not datacenters)
   - Cost: $0.50-2.00 per GB
   - Availability: Limited RU residential IPs
   - Setup: Rotating through many residential addresses

2. **Request Timing** (avoid bot signature):
   - Random delays: 5-30s between requests
   - Human-like behavior: Browse other pages
   - Session consistency: Same IP for duration

3. **Browser Fingerprint Matching**:
   - Timezone from IP location
   - Language headers from IP region
   - Device metadata consistency

**Current Gap**: No residential proxy integration

---

### LIGA STAVOK - QRATOR Bot Protection

**Current Status**: ⏳ Partially coded, QRATOR challenge blocks bootstrap

**What's Happening**:
```
Request: GET https://ligastavok.ru/
Response: QRATOR challenge page (JavaScript puzzle)
Required Action: Solve challenge or interact with page
Result: Cookie (access token) required for subsequent requests
```

**QRATOR Protection**:
- Behavioral analysis (mouse movement, typing patterns)
- Device fingerprinting
- JavaScript execution verification
- TLS/SSL anomaly detection

**Current Bootstrap Attempt**:
```rust
// From ligastavok.rs
async fn bootstrap_discovery(&self) -> Result<Bootstrap> {
    // Tries to fetch HTML bootstrap
    // But gets challenge page instead
}
```

**Required to Bypass**:
1. **Real Headless Browser**:
   - Full Chrome instance (not just navigation)
   - Real input simulation (keyboard, mouse)
   
2. **Selenium/Puppeteer Integration**:
   - More sophisticated than headless-chrome crate
   - Better anti-bot evasion
   
3. **2Captcha/AntiCaptcha Integration**:
   - If manual challenge solver needed
   
4. **User-Agent + Referer Fingerprint**:
   - Match referrer path to prevent suspicion
   - Consistent user-agent lifecycle

**Current Gap**: Only basic HTTP attempted, not interactive headless

---

## Part 6: ANTI-DETECTION INFRASTRUCTURE

### Available Stealth Modules

#### 1. **stealth.rs (parsers/src)**
```rust
pub struct StealthConfig {
    user_agents: Vec<String>,        // 4 different UAs
    viewports: Vec<(u32, u32)>,      // 5 viewport sizes
    languages: Vec<String>,           // Multiple locale strings
    platforms: Vec<String>,           // Win32, MacIntel, Linux x86_64
    screen_resolutions: Vec<(u32, u32)>,  // 3 resolutions
    color_depths: Vec<u32>,           // [24]
    pixel_ratios: Vec<f64>,           // [1.0, 1.25, 1.5, 2.0]
    timezones: Vec<String>,           // Europe/Moscow, UTC
}

pub fn get_headers(&self) -> HashMap<String, String> {
    headers.insert("Sec-Fetch-Dest", "document");
    headers.insert("Sec-Fetch-Mode", "navigate");
    headers.insert("Sec-Ch-Ua", "...");  // Chromium version
    headers.insert("Upgrade-Insecure-Requests", "1");
    // Total: 16 header variations
}
```

**Coverage**: Browser identification + HTTP header spoofing
**Gap**: No actual viewport/platform enforcement in network layer

#### 2. **StealthBetting (auto_betting/src)**
```rust
pub struct StealthBetting {
    user_agents: Vec<String>,         // 3 UAs
    random_delays: bool,              // 2-8s delays between requests
    min_delay_ms: u64,                // 2000ms
    max_delay_ms: u64,                // 8000ms
}

pub async fn wait_stealth(&self) {
    tokio::time::sleep(Duration::from_millis(delay)).await;
}
```

**Coverage**: Request timing randomization
**Gap**: Only applicable to sequential betting requests, not scanning

#### 3. **HeadlessChromeHelper (parsers/src)**
```rust
pub struct HeadlessChromeHelper {
    pub label: &'static str,
    pub user_agent: &'static str,
    pub platform: &'static str,
    pub viewport: (u32, u32),
    pub is_mobile: bool,
}

pub fn navigate_with_profile(&self, url: &str, profile: HeadlessProfile) {
    tab.set_user_agent(profile.user_agent, ...)?;
    tab.set_bounds(Bounds::Normal {
        width: Some(profile.viewport.0),
        height: Some(profile.viewport.1),
    })?;
}
```

**Coverage**: Browser profile emulation (viewport, UA, platform)
**Gap**: No DevTools protocol concealment

---

### Anti-Detection Gaps

| Detection Vector | Status | Implementation |
|---|---|---|
| User-Agent | ✅ Rotated | Randomized from 4-8 options |
| Viewport Size | ✅ Varied | 5 viewport sizes |
| Platform | ✅ Spoofed | Win32, MacIntel, Linux |
| Timezone | ✅ Set | Europe/Moscow, UTC |
| Language | ✅ Set | ru-RU, en-US variants |
| Screen Resolution | ✅ Set | 3 resolutions |
| Pixel Ratio | ✅ Set | 1.0 - 2.0 |
| **DevTools Detection** | ❌ Not hidden | `navigator.webdriver` still true |
| **Headless Flag** | ❌ Not hidden | `headless-chrome` detectable |
| **Chrome Args** | ❌ Visible | `--headless` in process list |
| **Mouse/Keyboard** | ❌ No simulation | Only synthetic events |
| **WebGL/Canvas** | ❌ No fingerprint | Could be detected by anti-bot |
| **Request Timing** | ⚠️ Partial | No inter-request delays in scanning |
| **Session Consistency** | ⚠️ New client each request | Could look bot-like |
| **Metadata Consistency** | ❌ No validation | Timezone/language/locale don't match |

---

## Part 7: PROXY INFRASTRUCTURE ANALYSIS

### ProxyManager Implementation

**Features**:
```rust
pub enum ProxyHealth {
    Healthy,      // >= 90% success rate, last check < 10s
    Degraded,     // 60-90% success rate, last check < 3s
    Unhealthy,    // < 60% success rate
    Banned,       // Explicitly failed repeatedly
}

pub struct ProxyManager {
    proxies: Vec<ProxyConfig>,
    health_by_proxy: HashMap<String, ProxyMetrics>,
    banned_since: HashMap<String, Instant>,
    current_index: AtomicU32,
}
```

**Supported Protocols**:
- HTTP
- HTTPS
- SOCKS5

**Rotation Strategy**:
```rust
pub fn get_next(&self) -> Option<String> {
    let idx = self.current_index.fetch_add(1, Ordering::SeqCst) as usize;
    let proxy = self.proxies[idx % self.proxies.len()];
    
    if !self.banned_proxies.contains(&proxy) {
        Some(proxy)
    } else {
        self.get_next()  // Try next
    }
}
```

**Health Tracking**:
```rust
pub fn success_rate(&self) -> f64 {
    self.success_count as f64 / (self.success_count + self.fail_count)
}

pub fn avg_response_time(&self) -> u64 {
    self.response_times.iter().sum() / self.response_times.len()
}

pub fn determine_health(&self) -> ProxyHealth {
    match success_rate {
        >= 0.9 => Healthy,
        0.6..0.9 => Degraded,
        _ => Unhealthy,
    }
}
```

**Timeout Configuration**:
- Healthy: 10s between checks
- Degraded: 3s between checks
- Unhealthy: 500ms (aggressive retry)
- Banned: 300s (cool-off period)

**Current Usage**:
- ✅ Olimp parser uses proxy rotation
- ✅ Circuit breaker integration done
- ❌ No residential proxy support (datacenter IPs only)
- ❌ No proxy provider integration (manual config only)

---

## Part 8: GAPS & RECOMMENDATIONS

### Critical Gaps

#### Gap 1: Headless Chrome Detection Prevention
**Impact**: 🔴 High - Blocks Winline, BetBoom, potentially others
**Current State**: No DevTools concealment
**Recommendation**:
```rust
// Add puppeteer-extra-stealth plugin equivalent in Rust
// OR use higher-level tool (playwright-rs instead of headless-chrome)

// Option A: Patch navigator properties
tab.evaluate(r#"
    Object.defineProperty(navigator, 'webdriver', {
        get: () => undefined,
    });
    chrome = {};  // Hide chrome object
"#)?;

// Option B: Switch to Playwright/Selenium
// More sophisticated anti-bot evasion built-in
```

---

#### Gap 2: Geoblocked Bookmaker Access (1xBet)
**Impact**: 🔴 High - Excludes 1xBet entirely
**Current State**: Only datacenter proxies available
**Recommendation**:
```rust
// Need residential proxy provider integration
// E.g., Bright Data, Smartproxy, OxyLabs

pub struct ResidentialProxyPool {
    provider: Box<dyn ProxyProvider>,  // API to request RU residential IPs
    pool_size: usize,                  // 50-100 active IPs
    rotation_interval: Duration,       // Change IP every 5-10 requests
}

// Cost: ~$200-500/month for 100GB RU traffic
// Result: Likely bypass 1xBet geolocking
```

---

#### Gap 3: WebSocket / Protobuf Decoding
**Impact**: 🟠 Medium - Unlocks BetBoom properly
**Current State**: Scaffolded but not implemented
**Recommendation**:
```rust
// 1. Capture real WebSocket traffic from browser
//    Use Chrome DevTools Network tab, export as HAR

// 2. Reverse-engineer protobuf schema
//    Tool: protobuf-inspector (analyze binary)

// 3. Generate Rust bindings
//    Command: protoc --rust_out=. schema.proto

// 4. Implement frame decoder
pub async fn subscribe_to_feed(&self, channel: &str) -> Result<EventStream> {
    let ws = tokio_tungstenite::connect(&self.ws_url).await?;
    
    // Send subscription request (protobuf serialized)
    let subscribe_msg = SubscribeRequest {
        channel: channel.to_string(),
        mode: "all".to_string(),
    };
    ws.send(Message::Binary(serialize_protobuf(&subscribe_msg)?))?;
    
    // Decode frames
    while let Some(frame) = ws.recv().await {
        let msg = decode_protobuf_frame(&frame)?;
        yield Event::from_protobuf(&msg);
    }
}

// Estimated effort: 20-40 hours
// Result: 6000+ BetBoom events unlocked
```

---

#### Gap 4: QRATOR Anti-Bot Bypass
**Impact**: 🟠 Medium - Unlocks Liga Stavok
**Current State**: Basic HTTP only
**Recommendation**:
```rust
// Option A: Real headless browser with interaction
// Use Selenium/Playwright instead of headless-chrome

pub async fn bypass_qrator(&self) -> Result<String> {
    let mut browser = Playwright::launch().await?;
    let page = browser.new_context().await?.new_page().await?;
    
    page.goto("https://ligastavok.ru/").await?;
    
    // Wait for QRATOR challenge
    page.wait_for_selector(".qrator-challenge").await?;
    
    // Simulate human interaction
    page.click("button").await?;  // Or wait for auto-solver
    
    page.wait_for_url("https://ligastavok.ru/api/").await?;
    
    Ok(page.content().await?)
}

// Cost: Playwright integration (~5 hours)
// Result: Liga Stavok bootstrap unlocked
```

---

#### Gap 5: Request Timing Randomization
**Impact**: 🟡 Low-Medium - Reduces bot signatures
**Current State**: Only in StealthBetting (not scanning)
**Recommendation**:
```rust
// Add inter-request delays to scanning pipeline

pub struct ScannerTiming {
    base_delay_ms: u64,              // 1-2 seconds
    random_variance: f64,            // 0.5-1.5x variance
    burst_limit: usize,              // Max 3 requests before delay
    per_bookmaker_spacing: Duration, // 5-10s between different BKs
}

pub async fn fetch_with_timing(&self, parser: &Parser) -> Result<Events> {
    self.wait_stealth_delay().await;
    
    let events = parser.fetch().await?;
    
    // Add spacing before next bookmaker
    tokio::time::sleep(self.per_bookmaker_spacing).await;
    
    Ok(events)
}

// Cost: ~2 seconds per parser (network I/O overlap can hide)
// Result: Reduced bot detection scores
```

---

#### Gap 6: Metadata Consistency Validation
**Impact**: 🟡 Low - Improves evasion quality
**Current State**: UA/viewport set independently
**Recommendation**:
```rust
// Ensure all metadata is coherent

pub struct ConsistentProfile {
    user_agent: String,              // From real Chrome versions
    timezone: String,                // Match IP geolocation
    language: String,                // Match timezone
    platform: String,                // Match UA (Win/Mac/Linux)
    viewport: (u32, u32),           // Common resolution for platform
    screen_resolution: (u32, u32),  // >= viewport size
    color_depth: u32,               // Typical: 24 or 32
}

pub fn validate_consistency(&self) -> Result<()> {
    // Windows UA must have Win32 platform
    // Mac UA must have MacIntel platform
    // Russian timezone with en-US language = suspicious
    // Desktop UA with mobile viewport = suspicious
}

// Cost: ~8 hours for implementation and matrix
// Result: More convincing fingerprints
```

---

### Medium-Term Improvements

#### 1. Parser Parity Across Bookmakers
**Current**: 7 working + 8 rollout-ready + 6 partial = 21/35 usable
**Goal**: Get Winline, BetBoom, Liga Stavok to working state
**Effort**: 60-80 hours (Winline: 30h, BetBoom: 20h, Liga: 15h)
**Gain**: +2700 additional events (Winline: 3000, BetBoom: 6000, Liga: pending)

#### 2. International Bookmaker Expansion
**Current**: 3 international (SBObet, 1xBet Alt, Betscope) via international_bundle
**Candidates**: 
- Bet365 (requires heavy anti-bot)
- Pinnacle (open API)
- SmarketsBet (API available)
**Effort**: 30-40 hours per new bookmaker
**Gain**: +3000-5000 events per bookmaker

#### 3. WebSocket Real-Time Feed
**Current**: HTTP polling only
**Candidates**: BetBoom (protobuf), Winline (WS direct)
**Benefits**:
- Real-time odds updates (100ms vs 30s)
- Fewer total requests (bandwidth/detection reduction)
- Better arbitrage window detection
**Effort**: 40-60 hours

#### 4. Distributed Scanning
**Current**: Single-machine agent swarm
**Goal**: Multi-IP, multi-location scanning
**Infra Needed**:
- Residential proxy pool (50-100 IPs)
- Distributed coordination (Redis/RabbitMQ)
- Geolocation IP database
**Cost**: $200-500/month proxy + $50-100/month infra
**Benefit**: Bypass per-IP rate limits entirely

---

## Part 9: IMPLEMENTATION ROADMAP

### Immediate (Week 1-2)

- [ ] Run Winline extraction JS against live page, capture actual selectors
- [ ] Fix Winline selector mismatches (high-priority)
- [ ] Set up BetBoom WebSocket frame capture (with browser DevTools)
- [ ] Document protobuf schema for BetBoom feed

### Short-term (Week 3-4)

- [ ] Implement DevTools protocol concealment for Winline
- [ ] Prototype BetBoom protobuf decoder
- [ ] Test Liga Stavok with Playwright-rs
- [ ] Add inter-request delay staggering to scanner

### Medium-term (Month 2)

- [ ] Complete BetBoom protobuf integration
- [ ] Unlock Winline WebSocket feed (alternative to JS extraction)
- [ ] Integrate residential proxy provider API
- [ ] Resolve 1xBet geoblocking with RU residential IPs

### Long-term (Month 3+)

- [ ] Multi-location distributed scanning
- [ ] Real-time WebSocket feeds for all major parsers
- [ ] Expand international bookmakers (+5 new)
- [ ] Machine learning for bookmaker website changes detection

---

## Part 10: TESTING & VALIDATION

### Current Test Suite

**Tests Implemented**:
- ✅ Event bus (8 tests)
- ✅ Calculator (39 tests)
- ✅ Cross-BK matching (9 diagnostic tests)
- ✅ Circuit breaker (3 tests)
- ✅ Proxy manager (health tracking)

**Test Coverage by Parser**:
| Parser | Unit Tests | Integration | Live Test |
|--------|-----------|-------------|-----------|
| Pari | Yes | Yes | ✅ |
| Fonbet | Yes | Yes | ✅ |
| Marathon | Yes | Yes | ✅ |
| Bettery | Yes | Yes | ✅ |
| Leon | Yes | Yes | ✅ |
| Winline | Partial | No | ❌ (0 events) |
| BetBoom | Partial | No | ⚠️ (scaffolded) |
| Olimp | Yes | Limited | ⚠️ (proxy needed) |

### Validation Commands

```bash
# Test individual parser
cargo test --lib parsers::pari

# Test circuit breaker
cargo test --lib parsers::circuit_breaker

# Full parser factory tests
cargo test --lib parsers::parser_factory

# Integration tests (requires network)
cargo test --test integration -- --ignored

# Live parser health check
./target/release/fork_hunter_bin health --parser winline
```

---

## Conclusion

**Overall Assessment**: Fork Hunter Pro has a **solid foundation** with 7 production parsers covering ~35,000 events. The infrastructure for handling blocking (proxies, circuit breaker, headless Chrome) is in place but needs refinement.

**Key Blockers**:
1. **Headless detection** (Winline, BetBoom)
2. **Geoblocking** (1xBet)
3. **Bot protection** (Liga Stavok QRATOR)
4. **Protobuf decoding** (BetBoom WebSocket)

**Quick Wins** (highest ROI):
1. Fix Winline selectors (1-2 hours, +3000 events)
2. Implement BetBoom protobuf (20 hours, +6000 events)
3. Add residential proxies (5 hours setup, unlocks 1xBet)

**Strategic Focus**: Focus on HTTP APIs first (easier to maintain), leave headless Chrome for specialized cases.

