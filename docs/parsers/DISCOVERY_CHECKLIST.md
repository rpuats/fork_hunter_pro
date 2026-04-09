# Bookmaker discovery checklist

Use this before promoting any missing bookmaker into the Rust parser factory.

## 1. Classify the acquisition path

Pick one primary class up front:

- **HTTP API** — stable direct JSON endpoint, reproducible without browser state
- **Bridge** — browser/bootstrap required, but parser can consume structured output from a helper
- **Browser interception** — site is SPA/protected, but JSON responses can be captured reliably
- **Do not proceed yet** — only DOM scraping ideas exist, or anti-bot posture is unclear

If the class is unknown, stop and collect evidence first.

## 2. Minimum evidence package

Required before implementation:

- target URLs for live and prematch
- at least one real captured response containing event identity + odds
- auth/cookie/header notes
- response size and pagination strategy
- known rate/refresh behavior
- sample of 1X2 market mapping

Recommended artifact locations:

- `network_capture/<bookmaker>_*.json`
- `docs/parsers/PARSER_COVERAGE.md` update
- focused discovery script in `scanner/parsers/` or `temp_*` only while still exploratory

## 3. Decide whether it belongs in Rust now

Promote to Rust only if one of these is true:

- direct HTTP contract is stable enough to parse without browser runtime, or
- bridge contract is explicit and narrow, with deterministic JSON handoff

Keep it out of `ParserFactory` if:

- payload shape is still speculative
- only DOM selectors work
- anti-bot gating is the main obstacle and there is no legitimate browser-assisted path yet

## 4. Normalization requirements

Before a parser is considered onboarded, confirm:

- canonical slug is final
- bookmaker name is consistent across Rust/Python/docs
- event IDs are deterministic enough for dedupe
- 1X2 mapping is explicit
- totals/handicaps are either mapped correctly or intentionally skipped
- live vs prematch is distinguishable
- sport detection has a fallback

## 5. Test expectations

Minimum:

- factory lookup test by slug
- parser smoke test against saved fixture or mocked payload
- assertion that empty/invalid payloads fail gracefully

Preferred:

- fixture test for event extraction
- fixture test for odds extraction
- regression test for any previously broken payload shape

## 6. Safety rules

- Do not add anti-bot bypass gimmicks as productized parser logic.
- Prefer browser-assisted interception over fake stealth layers when browser state is genuinely required.
- Keep discovery scripts separate from production parser code.
- Document blockers honestly instead of hiding them behind brittle fallbacks.

## 7. Merge gate for new parser registration

Before adding a slug to Rust `ParserFactory`, verify all are true:

- [ ] canonical slug confirmed
- [ ] acquisition class documented
- [ ] payload source documented
- [ ] at least one reproducible sample captured
- [ ] parser returns normalized `Event` / `Odd`
- [ ] focused test added
- [ ] fallback/disabled status documented if partial
