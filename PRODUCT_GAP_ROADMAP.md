# fork_hunter_pro — product gap audit and execution roadmap

_Last updated: 2026-04-09_

## Why this doc exists

User direction for the next product step is clear:
- more parser APIs
- competitor-parity functionality
- autobetting
- freebet wagering assistant
- min/max stake checks
- bankroll/deposit guidance

The repo already contains pieces of almost all of those themes, but they are unevenly implemented across:
- **Rust workspace** (`crates/`) — mainline runtime
- **Desktop UI** (`desktop-ui/`) — operator shell, mostly mock/static
- **Legacy Python** (`scanner/`, `automation/`, `freebet/`) — wider parser/automation experimentation, but fragmented

This roadmap converts that gap into a concrete delivery order.

---

## Executive summary

### What is already real enough to build on

**Rust / mainline**
- Multi-crate workspace already exists for scanner, parsers, API, bankroll, bonus, auto-betting.
- `scanner::GhostScanner` already wires together surebets, corridors, express forks, bankroll, bonus hunting, freebet hunting and autobet engine.
- `parsers` has a unified `BookmakerParser` trait with `fetch_events`, `fetch_odds`, `fetch_all`.
- `api` already exposes scanner status, surebets, freebets, generosity, corridors, express forks and bookmakers.
- `auto_betting`, `bonus_hunter`, `bankroll_manager` are present as dedicated crates.

**Python / legacy**
- Much broader parser experimentation surface: many bookmaker parser variants, Playwright/API/intercept prototypes, factory pattern, browser betting skeletons.
- Existing legacy autobet/browser control flow is useful as reference material for real execution adapters.

**Desktop UI**
- Solid shell/navigation exists for dashboard, surebets, corridors, express forks, history, settings.
- Current UI is still mostly mock-fed; good enough to become the first operator console once real API bindings are added.

### Main gap

The repo has **feature islands**, not a finished product lane:
- parser coverage exists, but parser capability is not surfaced as a first-class catalog/API
- autobet engine records placements, but does **not** execute against bookmaker adapters or verify market/stake constraints
- freebet logic exists, but is still calculator/planner level, not a wagering assistant with rules, progress and candidate selection
- bankroll management exists, but not yet a decision service for exposure, deposits, and per-bookmaker allocation
- desktop UI exposes settings and pages, but most controls are not connected to stateful backend actions

### Recommended product order

1. **Parser/platform foundation**
2. **Execution safety layer (limits, stake rules, verification)**
3. **Manual→semi-auto betting flow**
4. **Freebet wagering assistant**
5. **Bankroll + deposit guidance**
6. **Competitor-parity polish and higher automation**

That order minimizes rework. Building autobet before stake/rules verification or bankroll allocation will create unsafe churn.

---

## Current-state audit by surface

## 1) Rust mainline

### 1.1 Parsers

**What exists**
- `crates/parsers/src/base.rs` defines a stable parser trait.
- Rust parser set includes: `winline`, `pari`, `betcity`, `marathon`, `zenit`, `baltbet`, `bettery`, `fonbet`, `leon`, `olimp`, `sportbet`, `bet24`.
- Factory and circuit breaker exist.

**What is missing for the user goal**
- No explicit parser capability model: API/live/pre-match/auth-required/limits-supported/market-depth.
- No parser registry endpoint for operator use.
- No standardized raw-market schema versioning or parse diagnostics contract.
- No visible parser QA scoreboard that tells which bookmakers are production-ready versus experimental.
- No clear import path from strong legacy Python parser variants into Rust production adapters.

**Assessment**
- Good base for “more parser APIs”, but missing the productization layer that makes parser growth manageable.

### 1.2 Scanner / discovery

**What exists**
- `GhostScanner` already orchestrates parsers, normalization, surebet calculation, freebets, generosity, value, corridors, express, bonus hunter, bankroll manager, autobet engine.
- Parallel parser fetch with timeout and circuit breaker exists.

**Gaps**
- Cycle still truncates processing (`MAX_EVENTS_FOR_CALC = 500`, odds slice to 10k) without explicit adaptive prioritization.
- No ranking pipeline for “bettable first” opportunities.
- No execution-oriented enrichment per surebet: bookmaker account availability, min/max stake, expected fill confidence, time-to-expiry, liquidity/limit risk.

**Assessment**
- Scanner can find things; it is not yet an execution-grade decision engine.

### 1.3 API

**What exists**
- Endpoints for metrics, scanner status, surebets, freebets, generosity, history, corridors, express forks, bookmakers.

**Gaps**
- No API for parser catalog/status by capability.
- No API for autobet queues, dry-run validation, execution approval, or execution results.
- No API for bankroll recommendations / deposit transfers / account exposure heatmap.
- `get_bookmakers()` is currently hardcoded/static, not sourced from live parser health/runtime state.
- No endpoint family for freebet plans, wagering progress, candidate bets, rejected candidates, rule mismatches.

**Assessment**
- API is enough for dashboard demos, not enough for operator workflows.

### 1.4 Auto-betting

**What exists**
- `AutoBetEngine` tracks config, limits, history, stop/start, P/L and simple placement recording.
- `BetExecutor` exists.
- Rate limiting and stealth delay concepts exist.

**Critical gaps**
- No bookmaker-specific execution adapter interface.
- No pre-bet validation against live odds drift, market availability, min/max stake, step size, coupon constraints.
- Placement is currently logical/internal, not actual bookmaker execution.
- No two-leg orchestration policy for partial fills, hedging fallback, cancellation rules, rescue flow.
- No approval workflow: manual, one-click confirm, semi-auto threshold, emergency fallback.

**Assessment**
- Present crate is a **simulation/control skeleton**, not true autobetting yet.

### 1.5 Freebet / bonus

**What exists**
- `bonus_hunter::BonusCalculator` computes real value, EV, difficulty and progress.
- `BonusPlanner` builds a step plan.
- Scanner already references `engine::freebet::FreebetHunter`.
- Legacy Python has a tiny freebet recommender.

**Gaps**
- No unified “freebet wagering assistant” product flow:
  - import/track freebet inventory
  - encode bookmaker-specific bonus rules
  - recommend qualifying bets and conversion bets
  - track wagering progress from placed bets
  - explain why a candidate qualifies or fails
- Planner steps are generic placeholders, not linked to actual opportunities.
- No distinction between welcome bonus, cashback, insurance, freebet token, odds boost execution logic.

**Assessment**
- Strong starting math, weak operator workflow.

### 1.6 Bankroll / deposit guidance

**What exists**
- `BankrollManager` maintains balances/exposure and computes optimal stake via Kelly-style helper.
- Rebalance engine exists.

**Gaps**
- No deposit/withdrawal recommendation service using active opportunity demand.
- No “capital trapped in bookmaker A while needed in bookmaker B” prioritization.
- No account allocation per strategy: surebets vs corridors vs bonus wagering.
- No reserve policy / buffer per bookmaker / live vs prematch allocation.
- No UI/API for actionable bankroll operations.

**Assessment**
- Core math exists; product guidance layer does not.

---

## 2) Desktop UI

**What exists**
- Pages for dashboard, surebets, corridors, express forks, history, settings.
- Visual shell is already suitable for operator tooling.

**Gaps**
- `useScanner()` initializes with mock data and only partially hydrates from websocket.
- No screens for:
  - parser coverage/health
  - autobet queue / execution center
  - bankroll/account balances
  - freebet plans and wagering progress
  - rejected opportunities with reason codes
  - deposit guidance
- Settings page is mostly static inputs/toasts, not persisted backend config.

**Assessment**
- Desktop is currently a viewer shell, not a control plane.

---

## 3) Legacy Python

**What exists**
- Broad parser experimentation footprint across API, Playwright and interception variants.
- Parser factory with registry and health/circuit-breaker concepts.
- Two autobet prototypes:
  - `automation/auto_better.py` — workflow and confirmation semantics
  - `scanner/auto_better.py` — browser/API execution skeleton
- Minimal freebet helper.

**Gaps**
- Too much duplication and variant sprawl to treat as production runtime.
- Useful mostly as:
  - research archive
  - protocol reference
  - quick validation harness
  - migration source into Rust adapters

**Assessment**
- Python is a **capability mine**, not a scalable main product line.

---

## Product gap matrix

| Area | Rust today | Desktop today | Python today | Gap to target |
|---|---|---|---|---|
| Parser breadth | Medium | None | High | Need parser capability catalog + migration path + prod QA |
| Parser APIs surfaced to product | Low | None | Low | Need backend registry/status APIs and UI |
| Surebet scanning | Medium/High | Medium viewer | Medium | Need execution-grade ranking and filters |
| Competitor-parity operator tooling | Low/Medium | Low | Low | Need control-plane features, not just scanner pages |
| Autobetting | Low | Very low | Low/Medium prototype | Need adapter layer, validation, orchestration |
| Freebet wagering assistant | Low/Medium math | None | Low | Need rules engine + workflow + progress tracking |
| Min/max stake checks | Low | None | Very low | Need bookmaker constraints service |
| Bankroll/deposit guidance | Medium math | None | Low | Need decision engine + APIs + UI |

---

## Recommended module boundaries

These boundaries should be the target architecture, even if delivery is phased.

### A. `crates/parsers`
Owns:
- bookmaker fetch adapters
- parser capability metadata
- parser diagnostics and health
- normalized raw market extraction

Add:
- `ParserCapabilities`
- `ParserHealthSnapshot`
- `BookmakerConstraintSnapshot` (only if fetchable at parser stage)
- test fixtures per bookmaker

### B. `crates/engine`
Owns:
- normalization
- event matching
- surebet/corridor/express/freebet/value detection
- opportunity ranking
- pre-execution validation orchestration

Add submodules:
- `execution_ranker`
- `opportunity_enrichment`
- `constraint_matcher`
- `freebet_rules`

### C. `crates/auto_betting`
Owns:
- execution policies
- manual/semi-auto/full-auto modes
- execution queues
- two-leg orchestration
- partial-fill rescue policies
- audit log of execution attempts

Add:
- `adapter.rs` trait for bookmaker executors
- `validator.rs` for min/max/step/odds drift checks
- `orchestrator.rs` for leg ordering and failure handling
- `approval.rs` for confirm/dry-run/manual gates

### D. `crates/bankroll_manager`
Owns:
- balances
- exposure
- bankroll allocation
- deposit/withdraw recommendations
- strategy-level capital envelopes

Add:
- `allocation.rs`
- `recommendation.rs`
- `account_heat.rs`

### E. `crates/bonus_hunter`
Owns:
- bonus/freebet rule definitions
- EV and difficulty models
- wagering plans
- progress tracking
- candidate qualification logic

Add:
- `rules.rs`
- `freebet.rs`
- `qualifier.rs`
- `tracker.rs`

### F. `crates/api`
Owns:
- operator-facing APIs
- execution control endpoints
- parser status endpoints
- bankroll/freebet/autobet state exposure

Add endpoint groups:
- `/api/v1/parsers/*`
- `/api/v1/autobet/*`
- `/api/v1/bankroll/*`
- `/api/v1/freebet/*`
- `/api/v1/opportunities/*`

### G. `desktop-ui`
Owns:
- operator console
- review/approval UX
- health dashboards
- bankroll and freebet workflows

Add pages:
- Parser Health
- Execution Center
- Bankroll
- Freebet Assistant
- Opportunity Review

### H. `legacy-python/` role (conceptual only)
Owns:
- protocol research
- reverse-engineering notes
- migration fixtures
- emergency validation scripts

Not recommended to own production orchestration.

---

## Phased delivery roadmap

## Phase 0 — Stabilize the foundation (3-5 days)

**Goal**: make the current repo measurable enough for safe product expansion.

### Deliverables
- Create a live parser inventory generated from Rust, not README/static lists.
- Mark each bookmaker as: `prototype`, `usable`, `production_candidate`, `disabled`.
- Add a single source of truth doc for feature readiness per domain.
- Wire desktop/API bookmaker lists to runtime state instead of hardcoded arrays.

### Implementation notes
- Add `ParserCapabilities` + `ParserReadiness` enum in shared models.
- Make `ParserFactory` expose metadata in Rust.
- Replace hardcoded `get_bookmakers()` response with scanner/parser runtime snapshot.
- Add a small `docs/fixtures/` or `tests/fixtures/` convention for parser samples.

### Exit criteria
- Operator can see which bookmakers are really available.
- Repo stops overstating readiness in UI/API.

---

## Phase 1 — Parser API/productization lane (1-2 weeks)

**Goal**: satisfy “more parser APIs” in a maintainable way.

### Deliverables
- Parser catalog API.
- Per-bookmaker capability matrix:
  - prematch/live
  - events only / odds only / both
  - direct API / web / playwright / intercepted
  - auth needed
  - known stake constraints available or unknown
- Legacy Python → Rust migration backlog for strongest parser variants.
- Parser scorecard with freshness, event count, error rate, avg fetch time.

### Recommended order
1. shared capability structs
2. Rust parser metadata wiring
3. API endpoints
4. desktop parser-health page
5. migration tracker for Python-only strengths

### Exit criteria
- Adding a new bookmaker means filling metadata + tests, not tribal knowledge.
- “More parser APIs” becomes a visible product surface.

---

## Phase 2 — Pre-execution safety layer (1-2 weeks)

**Goal**: build the mandatory layer before true autobetting.

### Deliverables
- `auto_betting::validator` with checks for:
  - min stake
  - max stake
  - stake increment / rounding step
  - max payout / coupon exposure
  - odds drift tolerance
  - market still open / selection still present
- `BookmakerConstraints` model in shared.
- Constraint source strategy:
  - parser-sourced when available
  - config override when manual
  - runtime rejection reason if unknown
- Opportunity status tags:
  - `bettable`
  - `needs_manual_check`
  - `constraint_mismatch`
  - `stale_odds`

### Why this phase is mandatory
Without this, any autobet flow is fake-safe and will produce operational pain immediately.

### Exit criteria
- Every candidate can be dry-run validated before execution.
- Min/max stake checks exist as a first-class product feature.

---

## Phase 3 — Manual and semi-auto execution center (2 weeks)

**Goal**: convert autobetting from simulation into controlled execution.

### Deliverables
- Bookmaker execution adapter trait.
- First adapters for the 2-3 most stable bookmakers only.
- Execution queue with explicit states:
  - queued
  - validating
  - approved
  - placing_leg_1
  - placing_leg_2
  - hedging
  - settled_failed
  - aborted
- Manual approval UX in desktop.
- Semi-auto policy:
  - auto only above threshold
  - only on whitelisted bookmakers
  - only when constraints known
  - only when bankroll allocation permits

### Implementation notes
- Reuse Python autobet semantics as reference, not runtime dependency.
- Start with dry-run + simulated adapter, then enable one real adapter at a time.

### Exit criteria
- Operator can run manual and semi-auto flows with traceable outcomes.
- Autobetting is no longer just internal record creation.

---

## Phase 4 — Freebet wagering assistant (1-2 weeks)

**Goal**: turn existing bonus/freebet math into an actionable assistant.

### Deliverables
- Freebet inventory model:
  - bookmaker
  - amount
  - expiry
  - min odds
  - max stake/counting rules
  - sport/market restrictions
- Bonus/freebet rule engine.
- Candidate recommendation API returning:
  - recommended qualifying bets
  - recommended conversion bets
  - estimated conversion value
  - qualification reason / rejection reason
- Progress tracker tied to bet history.
- Desktop page for freebet plans and next steps.

### Important design point
Do **not** keep freebets as a side-note on surebets. Treat them as their own workflow with their own rules, candidate set and progress state.

### Exit criteria
- User can manage freebet turnover from a dedicated workflow.
- Assistant explains why a bet counts or does not count.

---

## Phase 5 — Bankroll and deposit guidance (1 week)

**Goal**: surface capital allocation as an operator decision service.

### Deliverables
- Per-bookmaker balance and exposure snapshot API.
- Deposit/withdraw recommendations based on:
  - recent opportunity demand
  - strategy allocation targets
  - minimum reserve buffers
  - pending execution load
- Suggested stake sizing by strategy profile:
  - conservative
  - balanced
  - aggressive
  - bonus-first
- Desktop bankroll page with account heatmap.

### Example output shape
- “Move/prepare +15k to Pari because current pending opportunity demand exceeds free balance.”
- “Do not autobet on Leon until balance buffer restored above X.”

### Exit criteria
- Bankroll guidance becomes operational, not theoretical.

---

## Phase 6 — Competitor-parity feature pack (2-3 weeks)

For this product, “competitor parity” should mean the practical feature set operators expect from serious arb tools — **not** overbuilding exotic ML first.

### Deliverables
- Opportunity quality/risk labels.
- Fast filters for bookmaker pairs, sports, live/prematch, min age, max drift risk.
- Execution-ready sorting (not just highest profit).
- Rejected/expired opportunity reason history.
- Account-aware opportunity suppression (hide non-bettable items).
- Session/watchlist/workspace presets for operator workflows.

### Nice-to-have after parity
- account-specific bookmaker limits learning
- fill-rate analytics by bookmaker
- strategy ROI breakdown
- smart routing of opportunities by execution confidence

### Exit criteria
- Product feels like an operator workstation, not just a scanner feed.

---

## Delivery order by codebase area

### First
- `shared`
- `parsers`
- `api`
- small desktop wiring fixes

### Second
- `auto_betting::validator`
- `bankroll_manager::allocation/recommendation`
- scanner enrichment hooks

### Third
- execution adapters
- execution center UI

### Fourth
- `bonus_hunter` rules/tracker/freebet workflow

### Fifth
- competitor-parity UX polish and analytics

---

## Immediate backlog (next 10 implementation tickets)

1. **Add parser capability/readiness structs to `shared::models`.**
2. **Expose real bookmaker inventory from Rust runtime instead of hardcoded API response.**
3. **Create `/api/v1/parsers` and `/api/v1/parsers/health` endpoints.**
4. **Create desktop Parser Health page and replace static bookmaker settings list.**
5. **Introduce `BookmakerConstraints` model with min/max/step/payout caps.**
6. **Implement `auto_betting::validator` dry-run validation returning reason codes.**
7. **Add opportunity enrichment fields: `bettable`, `constraint_status`, `execution_confidence`.**
8. **Create `/api/v1/autobet/dry-run` endpoint.**
9. **Create `bankroll_manager::allocation` + `/api/v1/bankroll/recommendations`.**
10. **Create `bonus_hunter::freebet` workflow objects + `/api/v1/freebet/plans`.**

If only **three** things can be done next, do **#2, #6, #9**.

---

## What should explicitly wait

Avoid spending the next sprint on:
- ML prediction layers
- aggressive full-auto execution across many bookmakers
- extra exotic opportunity types before stake/rule validation exists
- deeper UI polish before data/control APIs are real

These are tempting, but they delay the path to a trustworthy product.

---

## Final recommendation

Treat the repo as already having **70% of the nouns** and only **35-40% of the workflows**.

The winning move is not to add yet another isolated module. It is to connect existing Rust modules into a clear operator loop:

**parser capability → opportunity enrichment → stake/rule validation → bankroll check → manual/semi-auto execution → freebet/bonus progress → deposit guidance**

That is the shortest path from “powerful codebase” to “product that matches the user’s target direction.”
