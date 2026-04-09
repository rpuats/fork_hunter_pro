# fork_hunter_pro API / bot surface audit

Date: 2026-04-09
Scope: `crates/api`, `crates/bot`, `crates/fork_hunter_bin`, `crates/parsers`, `crates/auto_betting`, `crates/bankroll_manager`, `crates/bonus_hunter`, `desktop-ui`

## Executive summary

Current Rust surface is strong on **scanner read APIs** and **internal engines**, but thin on **operator control APIs**.

What already exists:
- scanner HTTP endpoints for health, metrics, surebets, freebets, generosity, history stats, corridors, express forks, bookmakers, websocket
- internal Rust engines for autobetting, bankroll management, bonus/freebet planning
- minimal Telegram bot with notifications plus `/start`, `/status`, `/help`

What is missing from public surface:
- parser coverage/health endpoint
- autobetting start/stop/emergency/status endpoints
- bankroll read/update/rebalance endpoints
- bonus plan CRUD/progress endpoints
- stake validation endpoint with min/max rules
- richer bot commands for operator actions

## Current API surface

Registered routes in `crates/api/src/routes.rs`:
- `GET /health`
- `GET /api/v1/metrics`
- `GET /api/v1/scanner/status`
- `GET /api/v1/surebets`
- `GET /api/v1/freebets`
- `GET /api/v1/analytics/generosity`
- `GET /api/v1/history/stats`
- `GET /api/v1/corridors`
- `GET /api/v1/express-forks`
- `GET /api/v1/bookmakers`
- `GET /api/v1/capabilities`  ← added in this audit as contract/discovery endpoint
- `GET /ws`

### Bot surface

Current Telegram commands in `crates/bot/src/telegram.rs`:
- `/start`
- `/status`
- `/help`

Current bot actions:
- send surebet notifications
- send system notifications

No command path currently reaches:
- `AutoBetEngine`
- `BankrollManager`
- `BonusHunter`

## Parser coverage audit

### Actively registered in Rust `ParserFactory`
- pari
- marathon
- bettery
- fonbet
- leon
- sportbet
- bet24

### Implemented but not active
- olimp
  - code exists in `crates/parsers/src/olimp.rs`
  - explicitly disabled in `ParserFactory` because payload normalization is incomplete

### Mentioned/exported/configured but not active in current Rust scanner surface
- winline
- betcity
- zenit
- baltbet

These are visible in module exports, config defaults, or legacy project material, but not wired into the live Rust `ParserFactory` during this audit.

## Planned endpoint contracts

These are the most aligned next additions based on existing crates.

### 1) Parser coverage / health

#### `GET /api/v1/parsers/coverage`
Returns parser registration + implementation status.

```json
{
  "success": true,
  "data": [
    {
      "slug": "pari",
      "status": "active",
      "parser_type": "api",
      "source": "crates/parsers/src/pari.rs",
      "events_last_cycle": 6608,
      "last_error": null
    }
  ],
  "error": null,
  "timestamp": "2026-04-09T09:00:00Z"
}
```

#### `GET /api/v1/parsers/health`
Should expose:
- `bookmaker`
- `status`
- `last_success`
- `last_error`
- `consecutive_failures`
- `avg_response_time_ms`
- `events_parsed`
- `uptime_percent`

The `ParserHealth` model already exists in `shared::models`.

### 2) Autobetting controls

#### `GET /api/v1/autobet/status`
Map directly from `AutoBetEngine::get_status()` + limiter stats.

```json
{
  "success": true,
  "data": {
    "enabled": true,
    "running": false,
    "emergency_stopped": false,
    "bets_placed_today": 0,
    "bets_placed_total": 0,
    "profit_today": 0.0,
    "profit_total": 0.0,
    "last_bet": null,
    "errors_today": 0,
    "limits": {
      "bets_this_hour": 0,
      "max_bets_per_hour": 10,
      "daily_stake": 0.0,
      "max_daily_stake": 100000.0,
      "remaining_daily": 100000.0
    }
  }
}
```

#### `POST /api/v1/autobet/start`
Starts engine.

Request body:
```json
{ "reason": "operator" }
```

#### `POST /api/v1/autobet/stop`
Stops engine.

#### `POST /api/v1/autobet/emergency-stop`
Triggers `AutoBetEngine::emergency_stop()`.

#### `GET /api/v1/autobet/history?limit=50`
Maps to `AutoBetEngine::get_history(limit)`.

### 3) Freebet / bonus planning

#### `GET /api/v1/bonuses`
Should list `BonusHunter::get_best_bonuses(limit)` or `get_all_active()`.

#### `POST /api/v1/bonuses/plans`
Request:
```json
{ "bookmaker": "pari" }
```
Creates plan via `BonusHunter::create_bonus_plan(bookmaker)`.

#### `GET /api/v1/bonuses/plans/:bookmaker`
Returns `BonusPlan` plus derived `next_step`.

#### `PATCH /api/v1/bonuses/plans/:bookmaker/progress`
Request:
```json
{ "wager_done": 12500.0 }
```
Maps to `BonusHunter::update_wager_progress(bookmaker, wager_done)`.

### 4) Bankroll / deposit guidance

#### `GET /api/v1/bankroll`
Maps to `BankrollManager::get_state()`.

#### `POST /api/v1/bankroll/balances`
Request:
```json
{
  "bookmaker": "pari",
  "balance": 25000.0,
  "exposure": 4000.0
}
```
Maps to `BankrollManager::update_balance()`.

#### `GET /api/v1/bankroll/rebalance`
Maps to `BankrollManager::get_rebalance_recommendations()`.

#### `POST /api/v1/bankroll/stake-advice`
Request:
```json
{
  "bookmaker": "pari",
  "edge": 0.05,
  "odds": 2.10
}
```
Maps to `BankrollManager::calculate_optimal_stake()`.

### 5) Stake min/max validation

#### `POST /api/v1/stakes/validate`
Not fully implemented in current crates yet, but this is the safest contract to anchor future UI.

Request:
```json
{
  "bookmaker": "pari",
  "stake": 2500.0,
  "profit_percent": 3.8,
  "total_stake": 5000.0,
  "context": "surebet"
}
```

Response:
```json
{
  "success": true,
  "data": {
    "accepted": true,
    "reason": null,
    "suggested_stake": 2500.0,
    "checks": {
      "hourly_limit": true,
      "daily_limit": true,
      "profit_threshold": true,
      "bookmaker_min": null,
      "bookmaker_max": null
    }
  }
}
```

Why `null` for bookmaker min/max now:
- current Rust code enforces global limiter rules only
- no bookmaker-specific min/max source exists yet
- contract can ship before those rules are discovered/implemented

## Desktop UI data needs

For operator-ready desktop UI, these fields are the real missing pieces:

### Must-have now
- surebet row identity and deep links
  - `surebet.id`
  - `surebet.legs[].url`
- parser diagnostics
  - `slug`
  - `status`
  - `parser_type`
  - `last_error`
  - `events_last_cycle`
- autobet safety state
  - `running`
  - `emergency_stopped`
  - limiter stats
- bankroll panel
  - per-bookmaker `balance`
  - `exposure`
  - `available`
  - `recommended_deposit`
  - `recommended_withdraw`
- bonus workflow
  - `progress_percent`
  - `wager_required`
  - `wager_done`
  - `next_step`

### Nice-to-have
- parser latency / uptime sparkline
- bonus EV ranking history
- autobet placement timeline
- action audit log

## Bot roadmap aligned with current crates

Recommended next Telegram commands:
- `/autobet_status`
- `/autobet_start`
- `/autobet_stop`
- `/autobet_stop_now`
- `/bankroll`
- `/rebalance`
- `/bonus_plan <bookmaker>`
- `/parsers`

These should call the same service layer as the future HTTP endpoints rather than re-implement logic in bot handlers.

## Changes made in this audit

1. Added `GET /api/v1/capabilities`
   - implemented in `crates/api/src/handlers.rs`
   - registered in `crates/api/src/routes.rs`
   - returns parser coverage summary, capability matrix, and desktop UI field requirements
2. Added this document:
   - `docs/API_SURFACE_AUDIT_2026-04-09.md`

## Recommended next safe build steps

1. Add `AppState` handles for:
   - `AutoBetEngine`
   - `BankrollManager`
   - `BonusHunter`
2. Implement read-only endpoints first:
   - `/api/v1/autobet/status`
   - `/api/v1/bankroll`
   - `/api/v1/bankroll/rebalance`
   - `/api/v1/bonuses`
   - `/api/v1/bonuses/plans/:bookmaker`
3. After that, add controlled write actions:
   - autobet start/stop/emergency-stop
   - bankroll balance updates
   - bonus progress updates

That order keeps risk low while giving desktop UI and bot enough real data to become operational.
