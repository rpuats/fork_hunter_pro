# Parser coverage audit

## Scope

This file tracks parser coverage across the current Rust workspace (`crates/parsers`) and the older Python stack (`scanner/parsers`).

Acquisition classes used here:

- **HTTP API** — direct reqwest/httpx style access to stable JSON endpoints.
- **Bridge** — Rust delegates to a Python/Playwright helper or similar subprocess bridge.
- **Browser interception** — page load is required, but the sustainable target is intercepted JSON/XHR/GraphQL, not brittle DOM scraping.
- **DOM fallback** — last resort only; do not promote this as the long-term path.

## Current Rust parser inventory

| Bookmaker | Rust module | In `ParserFactory` | Current acquisition | Notes |
|---|---|---:|---|---|
| Pari | `crates/parsers/src/pari.rs` | yes | HTTP API | One of the cleaner direct API implementations. |
| Marathon | `crates/parsers/src/marathon.rs` | yes | HTTP API | Good baseline for direct JSON parsers. |
| Bettery | `crates/parsers/src/bettery.rs` | yes | HTTP API | Network capture exists in `network_capture/bettery_api.json`. |
| Fonbet | `crates/parsers/src/fonbet.rs` | yes | HTTP API | Network capture exists in `network_capture/fonbet_api.json`. |
| Leon | `crates/parsers/src/leon.rs` | yes | HTTP API | Rich discovery history exists in `temp_leon_*`. |
| Sportbet | `crates/parsers/src/sportbet.rs` | yes | HTTP API | Appears actively used. |
| 24bet | `crates/parsers/src/bet24.rs` | yes | HTTP API | Canonical parser slug is `_24bet`; factory keeps `bet24` as legacy alias. |
| Winline | `crates/parsers/src/winline.rs` | no | Bridge | Rust shell-outs to `scanner/parsers/parse_winline_json.py`. Good migration candidate, but still bridge-bound. |
| Betcity | `crates/parsers/src/betcity.rs` | yes | HTTP API | Registered in `ParserFactory` and default runtime diagnostics; a fresh direct endpoint probe shows healthy live/prematch volume, so the zero-event nightly currently looks like transient noise rather than a structural feed blocker. |
| Baltbet | `crates/parsers/src/baltbet.rs` | yes | HTTP API | Registered in `ParserFactory`, included in default runtime diagnostics, and marked production-ready from strict nightly KPI evidence. |
| Zenit | `crates/parsers/src/zenit.rs` | yes | HTTP API | Registered in `ParserFactory` and default runtime diagnostics; readiness now records an earlier runtime pass but keeps Zenit below production because recent strict nightly runs regressed to zero events. |
| Olimp | `crates/parsers/src/olimp.rs` | yes | HTTP API | Re-enabled behind the direct `competitionsWithEvents` path; readiness now locks one bounded 2026-04-18 runtime probe with non-empty live/prematch event volume while production promotion stays gated. |

## Gaps with strongest repo evidence

These bookmakers have concrete discovery artifacts in the repo, but no active Rust registration yet.

| Bookmaker | Legacy evidence already in repo | Recommended class | Why this class |
|---|---|---|---|
| BetBoom | `scanner/parsers/betboom_intercept.py`, `temp_betboom_*`, `network_capture/betboom_api.json` | Browser interception / runtime-card fallback | Current evidence shows dynamic API calls behind page flow and compact runtime cards in rendered captures. Keep the Sporthub contract scaffold guarded, but now prefer the compact card fallback before declaring an empty result. |
| Liga Stavok | `scanner/parsers/ligastavok_api.py`, `scanner/parsers/ligastavok_playwright.py`, `network_capture/ligastavok_network.json` | Browser interception / bridge | Strong anti-bot/cookie gating (`qrator`, `lds-api-sites`). Runtime diagnostics now classify `ready`, `protection_only`, `header_only`, and `bootstrap_unavailable` bootstrap states instead of pretending protection cookies are a usable session. Sustainable path is browser-assisted bootstrap + intercepted API payloads, not raw bypass tricks. |
| 1xStavka | `scanner/parsers/onexstavka_parser.py`, `test_1xstavka_finder.py`, `1xstavka_*` artifacts | HTTP API if mirror/feed remains valid; otherwise browser interception | Legacy code already targets feed-style JSON. This looks like the best missing candidate for a direct parser if endpoint stability is confirmed. |
| Melbet | `scanner/parsers/melbet_intercept.py`, `temp_melbet_*` | Browser interception | Legacy notes already describe SPA/network interception as the realistic path. Avoid DOM-first parser work. |
| Pin-Up | `scanner/parsers/pinup_parser.py`, `explore_pinup.py` | Browser interception discovery first | Existing parser looks exploratory and assumes endpoint shapes. Needs real network capture before committing to HTTP API. |
| Tennisi | `scanner/parsers/tennisi_playwright.py`, `temp_tennisi_*` | Browser interception | Current implementation is DOM-oriented and brittle. The next durable step is response capture/classification. |

## Prioritized expansion order

1. **Re-enable existing Rust modules before adding new bookmakers**
   - `olimp`, `winline`
   - Reason: code already exists, so recovery cost is lower than net-new onboarding.
2. **1xStavka**
   - Best chance of becoming a clean direct HTTP parser.
3. **BetBoom**
   - Strong artifact trail; likely worth a dedicated browser-intercept parser layer.
4. **Liga Stavok**
   - High value, but anti-bot posture means more infra/bridge work.
5. **Melbet / Tennisi / Pin-Up**
   - Keep behind discovery-first workflow until a stable payload source is proven.

## Recommendations for sustainable architecture

### 1. Treat acquisition mode as a first-class concern
Do not group all parsers together as if they have the same operational risk. At minimum, keep these buckets visible in docs and tests:

- direct HTTP parsers
- bridge-backed parsers
- browser-intercept candidates
- disabled/experimental modules

### 2. Prefer JSON contract capture over DOM extraction
When a site requires a browser, the long-term objective should still be:

1. load page safely
2. intercept XHR/fetch/GraphQL/WebSocket payloads
3. identify stable response contracts
4. move parsing to normalized JSON handlers

### 3. Separate "discovery" from "production parser"
A good pattern for new bookmakers:

- `discovery` script / capture artifact
- parser contract notes (endpoint, auth, payload keys, refresh cadence)
- thin parser implementation using only proven inputs
- fixture-based test on captured payloads

### 4. Avoid unsafe bypass work
Do not add scraper code whose only purpose is to defeat anti-bot systems. If browser presence is required, classify it openly as bridge/interception and keep the browser in the loop.

## Concrete blockers observed during audit

- Rust `ParserFactory` does not currently expose all parser modules that exist in `crates/parsers/src`.
- `24bet` had a slug mismatch between parser implementation (`_24bet`) and factory registration (`bet24`).
- There is no single source of truth describing which bookmakers are:
  - implemented in Rust,
  - implemented only in legacy Python,
  - disabled due to payload complexity,
  - or still in discovery mode.
- Liga Stavok still depends on browser-assisted bootstrap or captured session material; the Rust parser now reports the blocker state explicitly, but the external protection posture remains the operational gate.

## Suggested next implementation tickets

1. Add a fixture-driven test harness for parser payload samples.
2. Promote `olimp` from sample-backed registration to broader live runtime validation and volume checks.
3. Introduce explicit parser metadata in Rust (slug, acquisition class, maturity, source-of-truth fixture path).
4. Build a lightweight browser-intercept bridge abstraction for candidates like BetBoom and Liga Stavok, instead of ad-hoc per-parser subprocess behavior.
