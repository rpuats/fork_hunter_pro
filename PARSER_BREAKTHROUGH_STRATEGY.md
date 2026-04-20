# PARSER BREAKTHROUGH STRATEGY - April 2026

## Status: Implementing DOM selector updates across all blocked parsers

### PRIORITY 1: Quick Wins (Highest ROI) - This Week

#### Winline (+3,000 events)
**Problem**: Old CSS selectors for event cards, teams, odds
**Current Selectors (Broken)**:
- `ww-feature-block-event-dsk` (old web component)
- `ww-feature-event-mini-card-dsk` (old)
- `.main-event` (old)

**New Selectors (Working)**:
- `.pinned-event` - Live/pinned events container
- `.pinned-event__match` - Match wrapper
- `.pinned-event__team` - Team names
- `.event-card` - Event card wrapper
- `.card` - Generic card
- `.coefficient-button` - Odds button (more specific)
- `.coefficient-button_fill` - Odds fill style
- `WW-FEATURE-EVENT-MARKET-DSK` - Market web component

**Action**: Update `HEADLESS_EXTRACT_JS` constant in `crates/parsers/src/winline.rs` with new selectors
**Expected Outcome**: 3,000-4,000 events/scan

---

#### BetBoom (+6,000 events)
**Problem**: WebSocket Protobuf decoding not implemented
**Current State**: 
- WebSocket connection established
- Binary frames recognized (`0x01` opcode for binary)
- No Protobuf decoder

**Solution Roadmap**:
1. Analyze captured Protobuf messages (see `crates/parsers/src/betboom.rs` lines 450-500)
2. Reverse-engineer message structure:
   - Event list format (betting markets, teams, odds)
   - Timestamp sync
   - Market updates
3. Implement Protobuf decoder (can use `prost` crate)
4. Stream events from WebSocket

**Quick Alternative**: Use HTTP API endpoints that may exist but aren't documented
- Probe: `https://betboom.ru/api/v{1-5}/markets`, `/events`, `/odds`
- Check network tab in headless for successful HTTP calls

**Expected Outcome**: 6,000-8,000 events/scan

---

### PRIORITY 2: Medium Effort - This Month

#### Liga Stavok (4,000+ events)
**Problem**: QRATOR JavaScript challenge blocking access
**Current Issue**: Bot detection after 5-10 requests
**Solution**:
1. Implement JavaScript challenge solver (challenge extraction + execution)
2. Add request throttling (1-2 sec between requests)
3. Use residential proxies to avoid geoblocking
4. Inject valid `__cf_bm` cookie from previous session

**Action Item**: Add QRATOR solver to `crates/parsers/src/liga_stavok.rs`
**Expected Outcome**: 4,000+ events/scan

---

#### мБет/Melbet (4,000+ events)
**Problem**: Struct field mismatches (commented out, needs schema fix)
**Current Issue**: 
- `Odd` struct expects `bookmaker_slug`, not `bookmaker`
- `Event` struct missing `name`, `timestamp` fields
- League is `Option<String>` not `String`

**Actions**:
1. Read correct `shared::Event` and `shared::Odd` schemas
2. Update struct initialization in both parsers
3. Map old field names to new ones
4. Test with 100-200 requests

**Files to Fix**:
- `crates/parsers/src/mbet.rs`
- `crates/parsers/src/melbet.rs`
- `crates/parsers/src/tennis.rs`

**Expected Outcome**: 8,000+ events/scan combined

---

### PRIORITY 3: Advanced - Next Sprint

#### 1xBet (blocked by IP geolocation)
**Problem**: 403 Forbidden on all non-Russian IPs
**Solution**: Residential proxy network + rotation
- Provider: SmartProxy, Bright Data, Oxylabs
- Cost: $200-500/month
- Benefit: +10,000 events minimum

**Current Blocker**: No residential proxy support in code
**Implementation**:
1. Extend `ProxyManager` to support residential proxies
2. Add proxy rotation logic with sticky sessions
3. Implement session cookie persistence
4. Monitor success rate (target: 80%+ successful requests)

---

### PRIORITY 4: API Discovery

#### Olimp, Olimpbet, BetCity, Baltbet
**Status**: Partially working, rate limiting issues
**Solution**: HTTP request pacing + circuit breaker optimization

#### International Bundle (SBObet, 1xBet Alt, Betscope)
**Status**: Infrastructure exists, needs market data mapping
**Action**: Map international market names to shared::Market schema

---

## Implementation Timeline

| Task | ETA | Effort | ROI |
|------|-----|--------|-----|
| Winline selectors update | 2 hours | Low | 3,000 events |
| BetBoom HTTP endpoint probe | 3 hours | Low | 6,000+ events |
| Liga Stavok QRATOR solver | 8 hours | Medium | 4,000 events |
| мБет/Melbet schema fix | 4 hours | Low | 8,000 events |
| 1xBet residential proxy | 20 hours | High | 10,000+ events |
| **TOTAL** | **37 hours** | **Medium** | **31,000+ events** |

---

## Key Metrics

**Current Working**: 35,000 events/day from 7 parsers
**Potential After Fixes**: 66,000+ events/day from 13+ parsers
**Growth**: ~85% throughput increase

**Status**: Achievable within 1 sprint (1-2 weeks)

---

## Code Quality Requirements

- [ ] All async operations have proper error handling
- [ ] Circuit breaker patterns for resilience
- [ ] Rate limiting with exponential backoff
- [ ] Proxy health checks before use
- [ ] Test coverage for selector changes
- [ ] Logging for debugging

---

## Testing Strategy

For each parser:
1. **Unit Tests**: Selector tests on sample HTML
2. **Integration Tests**: Full event extraction with timeout
3. **Performance Tests**: Events/second throughput
4. **Error Handling**: Network failures, timeouts, bot detection

---

Generated: 2026-04-20
Focus Area: Breaking through parser blockages
