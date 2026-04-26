# Requirements Document

## Introduction

Fork Hunter Pro v2 transforms the existing Rust-based sports arbitrage scanner (Ghost Imperium) into the leading arbitrage product on the Russian bookmaker market. The system currently covers 9 bookmakers with a working engine, HTTP API, React/TS desktop UI, and WebSocket. This upgrade adds: expanded parser coverage (Tennisi ≥3400 events, Olimp ≥3000 events), a hardened surebet/value/middles engine, semi-automated execution with ghost-mode anti-detection, proactive bankroll and freebet management, a bonus opportunities engine, full analytics and P&L tracking, multi-channel alerts, and a polished operator dashboard with mobile PWA support.

---

## Glossary

- **Scanner**: The Rust runtime pipeline that collects odds from bookmakers and feeds the engine.
- **Parser**: A bookmaker-specific Rust module that fetches and normalizes raw odds data.
- **Surebet / Вилка**: A set of bets across two or more bookmakers guaranteeing profit regardless of outcome.
- **Middle**: An overlapping handicap or total bet across bookmakers where both legs can win simultaneously.
- **Value Bet**: A bet where the estimated true probability exceeds the implied probability from the bookmaker's odds.
- **CLV (Closing Line Value)**: The difference between the odds at bet placement and the closing odds, used as a proxy for edge.
- **EV (Expected Value)**: The probability-weighted average outcome of a bet.
- **Ghost Mode**: A set of anti-detection techniques that make automated betting behaviour appear human.
- **Freebet**: A promotional bet token issued by a bookmaker that does not require the stake to be risked.
- **Bonus**: A bookmaker promotional offer (refund, early payout, boosted odds, etc.) that modifies the effective EV of a bet.
- **Operator**: The human user who monitors and controls the system.
- **БК (Букмекер)**: Russian bookmaker.
- **ЦУПИС**: Russian centralised payment processing system for bookmakers (4% tax applies).
- **Execution_Engine**: The Rust module responsible for staged bet placement (`crates/auto_betting`).
- **Bankroll_Manager**: The Rust module managing account balances and allocation (`crates/bankroll_manager`).
- **Freebet_Manager**: The subsystem tracking freebet lifecycle states.
- **Bonus_Engine**: The subsystem detecting and evaluating bookmaker bonus opportunities.
- **Middles_Engine**: The subsystem detecting middle opportunities.
- **Value_Engine**: The subsystem detecting value bets using sharp-line reference odds.
- **Analytics_Engine**: The subsystem computing P&L, CLV, and market intelligence metrics.
- **Alert_System**: The multi-channel notification subsystem.
- **Dashboard**: The React/TS operator-facing web UI.
- **PWA**: Progressive Web App — a web application installable on mobile devices with offline capability.

---

## Requirements

### Requirement 1: Tennisi Parser Coverage Uplift

**User Story:** As an Operator, I want the Tennisi parser to cover at least 3400 prematch events, so that I have maximum market coverage for arbitrage detection.

#### Acceptance Criteria

1. WHEN the diagnostics binary is executed with `--json-stdout tennisi`, THE Tennisi_Parser SHALL report a prematch event count of at least 3400.
2. WHEN the Tennisi API returns paginated results, THE Tennisi_Parser SHALL follow all pagination cursors until no further pages exist.
3. WHEN a Tennisi league endpoint returns events, THE Tennisi_Parser SHALL map each event to the correct `Sport` variant rather than `Sport::Other`.
4. IF the Tennisi primary endpoint returns an HTTP error, THEN THE Tennisi_Parser SHALL retry the request up to 3 times with exponential backoff before marking the endpoint as unavailable.
5. THE Tennisi_Parser SHALL discover and query all available league endpoints, including those not present in the initial category listing.
6. WHEN the diagnostics binary is executed with `--json-stdout tennisi`, THE Tennisi_Parser SHALL report a live event count of at least 200.

---

### Requirement 2: Olimp Parser Coverage Uplift

**User Story:** As an Operator, I want the Olimp parser to cover at least 3000 prematch events and 200 live events, so that Olimp is a fully productive bookmaker in the arbitrage pipeline.

#### Acceptance Criteria

1. WHEN the diagnostics binary is executed with `--json-stdout olimp`, THE Olimp_Parser SHALL report a prematch event count of at least 3000.
2. WHEN the diagnostics binary is executed with `--json-stdout olimp`, THE Olimp_Parser SHALL report a live event count of at least 200.
3. THE Olimp_Parser SHALL query the Olimp prematch REST API endpoint separately from the live endpoint.
4. WHEN the Olimp REST API is unavailable, THE Olimp_Parser SHALL fall back to the Olimp WebSocket feed for live events.
5. WHEN the Olimp mobile API returns additional events not present in the desktop API response, THE Olimp_Parser SHALL merge those events into the output without duplicates.
6. IF the Olimp_Parser receives a blocked or CAPTCHA response, THEN THE Olimp_Parser SHALL log the block event, pause for a configurable cooldown period, and resume with a rotated session.

---

### Requirement 3: Surebet Engine v2

**User Story:** As an Operator, I want the surebet engine to detect two-way, three-way, and cross-market arbitrage opportunities with accurate stake sizing, so that I can act on every profitable opportunity.

#### Acceptance Criteria

1. WHEN odds are available from two or more bookmakers for the same event, THE Surebet_Engine SHALL evaluate all two-way and three-way outcome combinations for arbitrage.
2. WHEN a 1X2 market and a Double Chance market exist for the same event across different bookmakers, THE Surebet_Engine SHALL evaluate cross-market arbitrage between them.
3. WHEN a Total market and an Individual Total market exist for the same event across different bookmakers, THE Surebet_Engine SHALL evaluate cross-market arbitrage between them.
4. WHEN a surebet is detected, THE Surebet_Engine SHALL compute the bookmaker margin for each leg and include it in the opportunity record.
5. WHEN stake sizing is requested, THE Surebet_Engine SHALL support Flat, Proportional, Kelly, and Dynamic Kelly allocation strategies selectable per session.
6. WHEN Kelly stake sizing is applied, THE Surebet_Engine SHALL cap the recommended stake at 5% of the current bankroll per leg to limit variance.
7. WHEN computed stakes would result in a rounding loss exceeding 0.05% of the expected profit, THE Surebet_Engine SHALL apply anti-rounding adjustment to the smaller stake.
8. THE Surebet_Engine SHALL complete one full detection cycle across all active bookmakers within 3 seconds.
9. WHEN a surebet opportunity is detected, THE Surebet_Engine SHALL assign a unique identifier and timestamp to the opportunity record.

---

### Requirement 4: Value Bet Engine

**User Story:** As an Operator, I want the value bet engine to identify bets with positive expected value using sharp bookmaker lines as reference, so that I can build long-term profit through value betting.

#### Acceptance Criteria

1. WHEN odds from Pinnacle or Bettery are available for an event, THE Value_Engine SHALL use those odds as the sharp reference line for EV calculation.
2. WHEN a sharp reference line exists, THE Value_Engine SHALL compute EV as `(BookmakerOdds / SharpOdds) - 1` for each outcome.
3. WHEN EV exceeds a configurable threshold (default 2%), THE Value_Engine SHALL emit a value bet opportunity record.
4. THE Value_Engine SHALL produce at least 50 value bet opportunities per day under normal market conditions.
5. WHEN a value bet is placed and the closing odds are available, THE Analytics_Engine SHALL compute CLV as `(PlacedOdds / ClosingOdds) - 1` and attach it to the bet record.
6. WHEN a value bet opportunity is created, THE Value_Engine SHALL assign a confidence score from 1 to 10 based on the sharpness of the reference line, the EV magnitude, and the time to match start.
7. IF no sharp reference line is available for an event, THEN THE Value_Engine SHALL not emit a value bet opportunity for that event.

---

### Requirement 5: Middles Engine

**User Story:** As an Operator, I want the system to detect middle opportunities on overlapping handicaps and totals, so that I can profit from scenarios where both legs win simultaneously.

#### Acceptance Criteria

1. WHEN handicap or total markets from two bookmakers overlap in value for the same event, THE Middles_Engine SHALL detect and record the middle opportunity.
2. WHEN a middle opportunity is detected, THE Middles_Engine SHALL compute the win-win scenario probability, the loss-win scenario probability, and the expected value.
3. WHEN a middle opportunity is detected, THE Middles_Engine SHALL compute the maximum profit (both legs win) and maximum loss (neither leg wins) in absolute currency units.
4. THE Middles_Engine SHALL produce at least 20 middle opportunities per day under normal market conditions.
5. WHEN a middle opportunity is detected, THE Middles_Engine SHALL assign a unique identifier and timestamp to the opportunity record.
6. WHEN two markets are correlated (e.g., match winner and Asian handicap on the same team), THE Middles_Engine SHALL flag the opportunity as correlated and exclude it from the standard arbitrage pipeline.

---

### Requirement 6: Correlated Markets Detection

**User Story:** As an Operator, I want the system to detect and block arbitrage on correlated outcomes, so that I do not place bets that appear to be arbitrage but carry hidden correlated risk.

#### Acceptance Criteria

1. WHEN two bet legs in a surebet or middle share the same underlying outcome (e.g., Team A wins and Team A -0.5 handicap), THE Surebet_Engine SHALL mark the opportunity as correlated and exclude it from the active opportunity list.
2. THE Surebet_Engine SHALL maintain a configurable correlation rule set covering at least: match winner vs Asian handicap same team, over/under total vs individual total same team, and draw no bet vs 1X2 draw.
3. WHEN a correlated opportunity is blocked, THE Surebet_Engine SHALL log the block reason and the two correlated market identifiers.

---

### Requirement 7: Semi-Auto Execution Flow

**User Story:** As an Operator, I want a staged execution flow with manual checkpoints, so that I can review and cancel bets at any stage before they are placed.

#### Acceptance Criteria

1. THE Execution_Engine SHALL implement the following execution stages in order: Pending → Prepared → Validated → Executing → Confirmed.
2. WHEN an opportunity enters the Pending stage, THE Execution_Engine SHALL display it in the operator queue with computed stakes and expected profit.
3. WHEN the Operator approves a Pending opportunity, THE Execution_Engine SHALL advance it to Prepared and pre-fill the stake amounts.
4. WHEN an opportunity is in the Prepared stage, THE Execution_Engine SHALL validate that current odds have not moved beyond a configurable tolerance (default 0.5%) before advancing to Validated.
5. IF odds have moved beyond tolerance during validation, THEN THE Execution_Engine SHALL return the opportunity to Pending and notify the Operator of the odds change.
6. WHEN an opportunity reaches the Executing stage, THE Execution_Engine SHALL place bets on all legs sequentially within 10 seconds of stage entry.
7. WHEN all legs are confirmed by the bookmaker, THE Execution_Engine SHALL advance the opportunity to Confirmed and record the actual placed odds and stakes.
8. WHEN the Operator issues a cancel command at any stage before Executing, THE Execution_Engine SHALL halt the opportunity and record the cancellation reason.
9. THE Execution_Engine SHALL complete the full Pending → Confirmed flow within 10 seconds of Operator approval.

---

### Requirement 8: Ghost Mode Anti-Detection

**User Story:** As an Operator, I want the execution engine to mimic human betting behaviour, so that bookmakers do not flag or restrict the accounts used for arbitrage.

#### Acceptance Criteria

1. WHEN placing a bet, THE Execution_Engine SHALL introduce a randomised delay between 800ms and 4000ms before submitting each bet slip.
2. WHEN placing a bet, THE Execution_Engine SHALL randomise the stake amount by ±1–3% of the computed stake to avoid round-number patterns.
3. THE Execution_Engine SHALL rotate browser fingerprints (User-Agent, Accept-Language, screen resolution) across sessions using a configurable fingerprint pool of at least 10 profiles.
4. WHEN a session has placed more than a configurable number of bets (default 5) within a single bookmaker session, THE Execution_Engine SHALL rotate to a new session before placing the next bet.
5. WHEN a bookmaker account has been active for more than a configurable cooldown period (default 30 minutes), THE Execution_Engine SHALL pause that account and resume after the cooldown expires.
6. THE Execution_Engine SHALL simulate mouse movement trajectories using Bézier curves before clicking bet slip elements.
7. WHEN ghost mode is active, THE Execution_Engine SHALL report its current status via `GET /api/v2/health/ghost` including fingerprint rotation count, session count, and last cooldown timestamp.
8. WHILE ghost mode is enabled and the system is under continuous operation for 7 days, THE Execution_Engine SHALL produce zero bookmaker account flags as measured by account restriction events.

---

### Requirement 9: Smart Hedge

**User Story:** As an Operator, I want the system to automatically find a hedge bet when one leg of a surebet is placed but the second leg is rejected or the odds change, so that I minimise loss exposure.

#### Acceptance Criteria

1. WHEN a bet leg is placed successfully but a subsequent leg is rejected by the bookmaker, THE Execution_Engine SHALL immediately search for an alternative hedge leg across all active bookmakers.
2. WHEN an alternative hedge leg is found, THE Execution_Engine SHALL present it to the Operator with the revised profit/loss matrix before placing.
3. WHEN no alternative hedge leg is found within 30 seconds, THE Execution_Engine SHALL notify the Operator of the unhedged exposure and the recommended manual action.
4. WHEN odds on a pending leg change by more than the configured tolerance before placement, THE Execution_Engine SHALL recompute the optimal hedge stake and present the revised opportunity to the Operator.
5. THE Execution_Engine SHALL log all hedge events including original opportunity ID, rejected leg details, hedge leg found (if any), and final P&L outcome.

---

### Requirement 10: Browser Extension

**User Story:** As an Operator, I want a browser extension that injects calculated stakes into bookmaker bet slips, so that I can place bets with one click without manually entering amounts.

#### Acceptance Criteria

1. THE Browser_Extension SHALL support Chrome and Firefox browsers.
2. WHEN the Operator opens a bookmaker bet slip in the browser, THE Browser_Extension SHALL detect the bet slip and auto-fill the stake amount received from the Scanner via WebSocket or Native Messaging.
3. WHEN the stake is injected, THE Browser_Extension SHALL highlight the filled field to confirm injection to the Operator.
4. WHEN the Operator clicks the extension's "Place Bet" button, THE Browser_Extension SHALL submit the bet slip without requiring additional manual input.
5. THE Browser_Extension SHALL maintain a persistent WebSocket connection to the Scanner API and reconnect automatically within 5 seconds of disconnection.
6. IF the bookmaker page structure changes and the bet slip selector no longer matches, THEN THE Browser_Extension SHALL display a warning to the Operator and disable auto-fill for that bookmaker until the selector is updated.

---

### Requirement 11: Proactive Bankroll Manager

**User Story:** As an Operator, I want the bankroll manager to track balances across all bookmaker accounts and advise on optimal fund allocation, so that I always have sufficient funds where opportunities arise.

#### Acceptance Criteria

1. THE Bankroll_Manager SHALL maintain a registry of all bookmaker accounts with current balance, currency, and last-updated timestamp.
2. WHEN an account balance falls below a configurable minimum threshold, THE Bankroll_Manager SHALL emit a funding advisory event with the recommended deposit amount.
3. WHEN a funding advisory is emitted, THE Bankroll_Manager SHALL include a comparison of available funding methods ranked by commission rate and processing speed.
4. THE Bankroll_Manager SHALL compute an optimal allocation formula distributing the total bankroll across active bookmakers proportional to their historical opportunity frequency.
5. WHEN a bet is placed, THE Bankroll_Manager SHALL deduct the stake from the corresponding account balance in real time.
6. WHEN a bet is settled, THE Bankroll_Manager SHALL update the account balance with the net result.
7. THE Bankroll_Manager SHALL compute the ЦУПИС tax liability (4% of net winnings) and display it as a running total on the dashboard.
8. WHEN the Operator requests bankroll advice via `POST /api/v2/bankroll/allocate`, THE Bankroll_Manager SHALL return the recommended allocation within 200ms.

---

### Requirement 12: Freebet Lifecycle Automation

**User Story:** As an Operator, I want the system to automatically manage freebets from discovery through wagering to cashout, so that I maximise freebet conversion without manual tracking.

#### Acceptance Criteria

1. THE Freebet_Manager SHALL track each freebet through the following lifecycle states: Discovered → Qualifying → Qualified → Wagering → Completed → Expired → Lost.
2. WHEN a new freebet is detected, THE Freebet_Manager SHALL record the bookmaker, nominal amount, expiry date, and qualification requirements.
3. WHEN a freebet requires qualification (e.g., a minimum deposit bet), THE Freebet_Manager SHALL identify the optimal qualifying bet and add it to the Operator's action queue.
4. WHEN multiple freebets are in the Wagering state, THE Freebet_Manager SHALL prioritise wagering by ascending expiry date.
5. WHEN wagering a freebet, THE Freebet_Manager SHALL compute the optimal odds for wagering using the formula `EV = FreebetAmount × (1 - 1/Odds) × WinProbability` and select the odds that maximise EV.
6. THE Freebet_Manager SHALL achieve an average freebet conversion rate of at least 75% across all completed freebets.
7. WHEN a freebet expires without being wagered, THE Freebet_Manager SHALL log the expiry event and update the conversion rate metric.
8. THE Freebet_Manager SHALL expose freebet lifecycle state via `GET /api/v2/freebets/lifecycle`.

---

### Requirement 13: Bonus Opportunities Engine

**User Story:** As an Operator, I want the system to detect and evaluate bookmaker bonus opportunities and integrate them into arbitrage EV calculations, so that I capture additional profit from promotions.

#### Acceptance Criteria

1. THE Bonus_Engine SHALL detect and evaluate the following bonus types: Refund on 0:0, Early Payout Up by 2, Boosted Odds, Acca Insurance, Deposit Bonus, and Freebet Qualification.
2. WHEN a bonus opportunity is detected for an event that also has a surebet or value bet, THE Bonus_Engine SHALL compute the real EV including the bonus contribution and attach a "💎 Bonus Available" indicator to the opportunity record.
3. THE Bonus_Engine SHALL maintain a Bonus Calendar listing all active bonus programs with their activation conditions, expiry times, and applicable bookmakers.
4. WHEN a bonus program is scheduled to activate within 60 minutes, THE Alert_System SHALL send a push notification to the Operator.
5. WHEN a bonus opportunity is detected, THE Bonus_Engine SHALL record the nominal value, trigger probability, expected value, and expiry timestamp.
6. THE Bonus_Engine SHALL cover at least 5 distinct bonus types simultaneously.
7. WHEN the Operator requests the bonus calendar via `GET /api/v2/bonuses/calendar`, THE Bonus_Engine SHALL return all active and upcoming bonus programs within 100ms.

---

### Requirement 14: Bet Tracker and P&L Analytics

**User Story:** As an Operator, I want a complete bet history with automatic grading and P&L reporting, so that I can measure performance and optimise my strategy.

#### Acceptance Criteria

1. THE Analytics_Engine SHALL log 100% of placed bets including bookmaker, event, market, odds, stake, placement timestamp, and opportunity type (surebet, value, middle, freebet).
2. WHEN a bet result is available from the bookmaker, THE Analytics_Engine SHALL automatically grade the bet (win/loss/void/push) with accuracy of at least 90%.
3. THE Analytics_Engine SHALL compute P&L broken down by bookmaker, strategy type, time period (daily/weekly/monthly), and sport.
4. THE Analytics_Engine SHALL compute ROI, Yield, and ROC metrics and expose them via `GET /api/v2/analytics/pll`.
5. WHEN the Operator requests CLV analytics via `GET /api/v2/analytics/clv`, THE Analytics_Engine SHALL return CLV scores per bet and an aggregate CLV trend graph data set.
6. THE Analytics_Engine SHALL track odds movement from detection time to closing time for all monitored events.
7. WHEN the Operator applies a filter (bookmaker, sport, date range, strategy), THE Analytics_Engine SHALL return filtered P&L results within 500ms.

---

### Requirement 15: Arb Lifespan Predictor

**User Story:** As an Operator, I want the system to predict how long a detected arbitrage opportunity will remain valid, so that I can prioritise execution of short-lived opportunities.

#### Acceptance Criteria

1. WHEN a surebet opportunity is detected, THE Analytics_Engine SHALL compute a predicted lifespan in seconds based on opportunity age, market volatility, estimated liquidity, and time to match start.
2. WHEN the predicted lifespan is less than 60 seconds, THE Alert_System SHALL escalate the opportunity alert to high priority.
3. THE Analytics_Engine SHALL update the predicted lifespan every 5 seconds while the opportunity remains active.
4. WHEN an opportunity expires (odds move out of arbitrage range), THE Analytics_Engine SHALL record the actual lifespan and use it to improve future predictions for similar market conditions.

---

### Requirement 16: Multi-Channel Alert System

**User Story:** As an Operator, I want to receive arbitrage alerts through multiple channels, so that I never miss a profitable opportunity regardless of which device I am using.

#### Acceptance Criteria

1. THE Alert_System SHALL deliver surebet alerts via Telegram within 1 second of opportunity detection.
2. THE Alert_System SHALL support Discord webhook delivery for all alert types.
3. THE Alert_System SHALL support Browser Push Notification delivery for all alert types.
4. THE Alert_System SHALL support SMS delivery for high-priority alerts (predicted lifespan < 60 seconds).
5. THE Alert_System SHALL deliver a daily Email digest summarising P&L, opportunities found, and conversion rate.
6. WHEN an alert is delivered, THE Alert_System SHALL include the opportunity type, profit percentage, bookmakers involved, event name, and predicted lifespan.
7. WHEN the Operator configures alert preferences, THE Alert_System SHALL apply the configuration within 5 seconds without requiring a system restart.

---

### Requirement 17: Multi-Operator Mode

**User Story:** As an administrator, I want to support multiple operators sharing the same scanner with role-based access, so that a team can collaborate without interfering with each other's execution.

#### Acceptance Criteria

1. THE Dashboard SHALL support multiple named operator profiles each with an assigned bookmaker subset and bankroll view.
2. WHEN two operators attempt to execute bets on the same opportunity simultaneously, THE Execution_Engine SHALL detect the conflict and allow only the first operator to proceed, notifying the second of the conflict.
3. THE Dashboard SHALL display a leaderboard showing P&L and conversion rate per operator for the current period.
4. WHEN an operator's action would affect a shared bankroll account, THE Bankroll_Manager SHALL require confirmation from the account owner before proceeding.

---

### Requirement 18: Operator Dashboard v2

**User Story:** As an Operator, I want a redesigned dashboard with colour-coded opportunities and one-click execution, so that I can act on opportunities faster and with less cognitive load.

#### Acceptance Criteria

1. THE Dashboard SHALL colour-code surebet opportunities as follows: green (🟢) for profit ≥ 2%, yellow (🟡) for profit 1–2%, blue (🔵) for bonus opportunities, and white (⚪) for value bets.
2. WHEN the Operator clicks an opportunity row, THE Dashboard SHALL display a one-click execution panel with pre-filled stakes and a single confirm button.
3. THE Dashboard SHALL support drag-and-drop stake adjustment on the execution panel, updating the profit calculation in real time.
4. THE Dashboard SHALL include a Panic Button that, when activated, cancels all pending and prepared executions and closes all open bet slips within 5 seconds.
5. THE Dashboard SHALL load and render the initial opportunity list within 2 seconds on a standard broadband connection.
6. WHEN new opportunities arrive via WebSocket, THE Dashboard SHALL update the opportunity list without a full page reload.

---

### Requirement 19: Mobile PWA

**User Story:** As an Operator, I want a mobile-responsive Progressive Web App, so that I can monitor and act on opportunities from my smartphone.

#### Acceptance Criteria

1. THE Dashboard SHALL be installable as a PWA on iOS and Android devices.
2. WHEN the device is offline, THE Dashboard SHALL display the last cached opportunity list and indicate the offline state to the Operator.
3. THE Dashboard SHALL render correctly on screens with a minimum width of 375px using touch-optimised controls.
4. WHEN the PWA receives a push notification for a high-priority alert, THE Dashboard SHALL open the relevant opportunity detail on tap.
5. THE Dashboard SHALL achieve a Lighthouse Performance score of at least 80 on mobile.

---

### Requirement 20: Dark and Light Theme

**User Story:** As an Operator, I want to switch between a cyberpunk dark theme and a clean light theme, so that I can use the dashboard comfortably in different lighting conditions.

#### Acceptance Criteria

1. THE Dashboard SHALL provide a cyberpunk dark theme and a clean light theme selectable by the Operator.
2. WHEN the operating system theme preference changes, THE Dashboard SHALL automatically switch to the matching theme unless the Operator has manually overridden it.
3. WHEN the Operator switches themes, THE Dashboard SHALL apply the new theme within 200ms without a page reload.

---

### Requirement 21: API v2 Endpoints

**User Story:** As an integrator, I want a versioned v2 API exposing all new engine capabilities, so that external tools and the dashboard can consume structured data reliably.

#### Acceptance Criteria

1. THE API SHALL expose the following endpoints: `GET /api/v2/surebets`, `GET /api/v2/surebets/{id}/execute`, `GET /api/v2/middles`, `GET /api/v2/valuebets`, `GET /api/v2/bonuses`, `GET /api/v2/bonuses/calendar`, `POST /api/v2/bankroll/allocate`, `GET /api/v2/bankroll/advice`, `GET /api/v2/freebets/lifecycle`, `POST /api/v2/freebets/qualify`, `GET /api/v2/analytics/pll`, `GET /api/v2/analytics/clv`, `GET /api/v2/execution/queue`, `POST /api/v2/execution/panic`, `GET /api/v2/health/ghost`.
2. THE API SHALL respond to all GET endpoints at the 95th percentile within 100ms under normal load.
3. WHEN an invalid request is received, THE API SHALL return a structured JSON error response with an HTTP 4xx status code and a human-readable message.
4. THE API SHALL version all responses with a `X-API-Version` header set to `2.0`.
5. WHEN the Operator calls `POST /api/v2/execution/panic`, THE API SHALL forward the panic command to the Execution_Engine and return a confirmation within 1 second.

---

### Requirement 22: New Shared Data Models

**User Story:** As a developer, I want the shared models crate to define all new domain types, so that all crates share a single source of truth for data structures.

#### Acceptance Criteria

1. THE Shared_Models crate SHALL define `MiddleOpportunity` with fields: id, event_id, bookmaker_a, bookmaker_b, market_a, market_b, odds_a, odds_b, win_win_scenario, loss_win_scenario, expected_value, max_profit, max_loss.
2. THE Shared_Models crate SHALL define `BonusOpportunity` with fields: id, bonus_type, bookmaker, event_id, description, nominal_value, probability_trigger, expected_value, expires_at, requirements.
3. THE Shared_Models crate SHALL define `BonusType` as an enum with variants: RefundOnDrawZeroZero, EarlyPayoutUpByTwo, BoostedOdds, AccaInsurance, DepositBonus, FreebetQualification.
4. THE Shared_Models crate SHALL define `FreebetLifecycle` with fields: id, bookmaker, amount, status, qualification_progress, wagering_progress, expiry_date, expected_conversion_rate.
5. THE Shared_Models crate SHALL define `FreebetStatus` as an enum with variants: Discovered, Qualifying, Qualified, Wagering, Completed, Expired, Lost.
6. WHEN any crate serialises or deserialises a shared model to/from JSON, THE Shared_Models crate SHALL produce output that round-trips without data loss (parse → serialise → parse yields an equivalent value).
7. THE Shared_Models crate SHALL compile without warnings under `cargo build` with the `--all-features` flag.

---

### Requirement 23: System Performance and Reliability

**User Story:** As an Operator, I want the system to meet defined performance and reliability targets, so that I can depend on it during live market hours.

#### Acceptance Criteria

1. THE Scanner SHALL complete one full detection cycle across all active bookmakers within 3 seconds.
2. THE API SHALL respond to all GET endpoints at the 95th percentile within 100ms.
3. WHEN the Scanner process crashes, THE Scanner SHALL restart automatically within 10 seconds and resume the detection cycle.
4. THE Dashboard SHALL load and render the initial view within 2 seconds on a standard broadband connection.
5. WHEN the WebSocket connection between the Dashboard and the API is interrupted, THE Dashboard SHALL reconnect automatically within 5 seconds.
6. THE System SHALL log all errors to a structured log file with severity, timestamp, module, and message fields.
7. WHEN the `GET /api/v1/health` endpoint is called, THE API SHALL return a response within 50ms indicating the operational status of all subsystems.
