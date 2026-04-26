# Winline Parser - Status Report

## Current Situation

Winline website uses **Web Components** and **WebSocket** for real-time event updates. After extensive investigation:

### What We Found

1. **Static HTML**: Only SEO content (6500 bytes), NO actual events
2. **API Endpoints**: `/api/cls/*` and `/api/xds/v2/*` no longer work (returns PNG or 410)
3. **Main.js File**: 1.1MB bundled Angular app
4. **Web Components**: Root element is `<ww-app-dsk>` (Web Components container)
5. **WebSocket**: Events load via `wss://wss.winline.ru/data_ng?client=newsite&nb=true`
6. **Headless Browser**: Winline now detects headless browsers (EmulationException error experienced earlier)

### Key Discovery

**Events are delivered via WebSocket using BINARY protocol**, not standard JSON over HTTP.

Binary frame structure:
- Requires protocol decoder (likely msgpack or custom binary format)
- Not accessible through standard JavaScript network inspection in headless mode

### Proof of Working Events

File `winline_events_synced.json` contains 3500 real events from previous working parser:
- 10+ live football matches
- 3000+ prematch events (various sports)
- Shows the structure we need to implement

## Current Solution Status

### ✅ What Works

1. **Rust parser infrastructure created**
   - `crates/parsers/src/winline_static.rs` - Compiles successfully
   - Defines correct Event structure
   - Returns sample data proving structure works
   - Ready for WebSocket integration

2. **Python investigation complete**
   - Identified WebSocket URL
   - Identified Web Components pattern
   - Confirmed headless browser limitation
   - Network analysis complete

### 🔴 Blockers

1. **WebSocket Binary Protocol**: Unknown protocol format
   - Need to reverse-engineer from real browser
   - Requires Chrome DevTools with visible browser
   - Binary data cannot be easily inspected

2. **Headless Browser Detection**: Winline detects automated access
   - Attempts with Playwright headless: Blocked
   - Both regular and stealth mode fail
   - Requires real browser with human-like behavior

## Next Steps To Success

### Option 1: Real Browser Analysis (RECOMMENDED)
```
1. Open visible Chrome browser to https://winline.ru/stavki/sport/futbol/
2. Open DevTools (F12)
3. Go to Network tab → Filter by "WS" (WebSocket)
4. Find "data_ng" connection
5. Click on it → Go to "Messages" tab
6. Screenshot the binary frames or record first/last message
7. Decode binary format (likely protobuf or msgpack)
8. Implement binary decoder in Rust
```

### Option 2: Use Real Browser Wrapper (Harder but Possible)
```
1. Use Playwright with real (non-headless) browser
2. Let it connect to WebSocket naturally
3. Inject monitoring script to capture frames
4. Relay messages to Python server
5. Process in Python, send to Rust via HTTP bridge
```

### Option 3: Find Alternative API (Not Recommended)
```
1. Check if Winline has official API documentation
2. Reverse-engineer other Winline client apps (iOS/Android)
3. This may violate ToS
```

## Files for Reference

- **Rust**: `crates/parsers/src/winline_static.rs` (ready for WebSocket integration)
- **Python Investigation**: Multiple test files showing what was attempted
- **Evidence**: `winline_events_synced.json` (3500 real events from earlier)
- **Documentation**:
  - `PARSERS_FINAL_REPORT.md`
  - `WINLINE_WORKING_STRATEGY.md`
  - `IMMEDIATE_ACTION_PLAN.md`

## Technical Requirements

To make this work, need:
1. **Access to real browser** (Chrome, Firefox, or Safari - NOT headless)
2. **Ability to analyze WebSocket frames** (Chrome DevTools or similar)
3. **Binary format decoder** (likely protobuf, msgpack, or custom)
4. **Python websockets library** with binary frame support
5. **Rust tokio-tungstenite** for WebSocket in production

## User's Current Requirement

> "работающий парсер это парсер который вытащитл болле 10- лавйв событий +-3000 прематч"
> (Working parser = 10+ live + 3000 prematch events)

This requirement is **ACHIEVABLE** once WebSocket protocol is deciphered.

## Proof of Feasibility

The file `winline_events_synced.json` (dated 11.04.2026) proves:
- ✅ Website serves events
- ✅ Total of 3500+ events
- ✅ Includes 10+ live matches
- ✅ Includes 3000+ prematch events
- ✅ Structure matches what Rust parser expects

Therefore, the solution exists; we just need to find how to access it through WebSocket.
